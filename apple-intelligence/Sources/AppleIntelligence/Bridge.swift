import Foundation
import SwiftRs

#if canImport(FoundationModels)
import FoundationModels
#endif

// MARK: - JSON helpers

private func jsonOk(_ text: String) -> String {
    let obj: [String: String] = ["ok": text]
    guard let data = try? JSONSerialization.data(withJSONObject: obj),
          let json = String(data: data, encoding: .utf8) else {
        return #"{"ok":""}"#
    }
    return json
}

private func jsonErr(_ error: String, _ message: String) -> String {
    let obj: [String: String] = ["error": error, "message": message]
    guard let data = try? JSONSerialization.data(withJSONObject: obj),
          let json = String(data: data, encoding: .utf8) else {
        return #"{"error":"\#(error)","message":""}"#
    }
    return json
}

// Thread-safe container for passing results across Task boundaries in Swift 6.
private final class ResultBox: @unchecked Sendable {
    var value: String
    init(_ value: String) { self.value = value }
}

// MARK: - Availability check

@_cdecl("ai_check_availability")
public func aiCheckAvailability() -> Bool {
    #if canImport(FoundationModels)
    guard #available(macOS 26.0, *) else {
        return false
    }
    return SystemLanguageModel.default.availability == .available
    #else
    return false
    #endif
}

// MARK: - JSON payload types

private struct ChatPayload: Decodable {
    let system: String?
    let messages: [ChatMessage]
}

private struct ChatMessage: Decodable {
    let role: String  // "user" or "assistant"
    let content: String
}

// MARK: - Error mapping

#if canImport(FoundationModels)
/// Maps a Foundation Models generation error to the bridge's JSON error envelope.
@available(macOS 26.0, *)
private func generationErrorJSON(_ error: LanguageModelSession.GenerationError) -> String {
    switch error {
    case .guardrailViolation:
        return jsonErr("guardrail_violation", "Content flagged by safety guardrails")
    case .exceededContextWindowSize:
        return jsonErr("context_exceeded", "Context window size exceeded")
    case .assetsUnavailable:
        return jsonErr("assets_unavailable", "Model assets are not available")
    case .unsupportedLanguageOrLocale:
        return jsonErr("unsupported_language", "Language or locale is not supported")
    case .rateLimited:
        return jsonErr("rate_limited", "Rate limit exceeded")
    case .concurrentRequests:
        return jsonErr("concurrent_requests", "Too many concurrent requests")
    case .decodingFailure:
        return jsonErr("decoding_failure", "Model output could not be decoded")
    case .refusal:
        return jsonErr("refusal", "Model refused to generate content")
    case .unsupportedGuide:
        return jsonErr("unsupported_guide", "Requested generation guide is not supported")
    @unknown default:
        return jsonErr("generation_error", error.localizedDescription)
    }
}
#endif

// MARK: - Text generation

/// Synchronous wrapper around Foundation Models async API.
/// Uses DispatchSemaphore + Task to bridge async/sync boundary.
/// Must be called from a non-main thread (Tauri commands satisfy this).
@_cdecl("ai_generate")
public func aiGenerate(system: SRString, user: SRString) -> SRString {
    #if canImport(FoundationModels)
    guard #available(macOS 26.0, *) else {
        return SRString(jsonErr("unsupported_os", "macOS 26.0 or later is required"))
    }

    let systemPrompt = system.toString()
    let userPrompt = user.toString()

    let box = ResultBox(jsonErr("unknown", "Generation did not complete"))
    let semaphore = DispatchSemaphore(value: 0)

    Task {
        do {
            let session = LanguageModelSession(instructions: systemPrompt)
            let response = try await session.respond(to: userPrompt)
            box.value = jsonOk(response.content)
        } catch let error as LanguageModelSession.GenerationError {
            box.value = generationErrorJSON(error)
        } catch {
            box.value = jsonErr("unknown", error.localizedDescription)
        }
        semaphore.signal()
    }

    semaphore.wait()
    return SRString(box.value)
    #else
    return SRString(jsonErr("unsupported_sdk", "FoundationModels framework not available"))
    #endif
}

// MARK: - Multi-turn generation

/// Accepts a JSON payload with conversation history and replays it through a single
/// LanguageModelSession, enabling true multi-turn context.
///
/// Payload format: `{"system": "...", "messages": [{"role": "user", "content": "..."}, ...]}`
/// The last message must be from the user. Previous assistant messages are replayed
/// via `session.respond(to:)` to build up internal session state.
@_cdecl("ai_generate_with_history")
public func aiGenerateWithHistory(payload: SRString) -> SRString {
    #if canImport(FoundationModels)
    guard #available(macOS 26.0, *) else {
        return SRString(jsonErr("unsupported_os", "macOS 26.0 or later is required"))
    }

    let payloadStr = payload.toString()
    guard let data = payloadStr.data(using: .utf8),
          let chat = try? JSONDecoder().decode(ChatPayload.self, from: data) else {
        return SRString(jsonErr("invalid_payload", "Failed to parse JSON payload"))
    }

    guard !chat.messages.isEmpty else {
        return SRString(jsonErr("empty_messages", "Messages array is empty"))
    }

    let box = ResultBox(jsonErr("unknown", "Generation did not complete"))
    let semaphore = DispatchSemaphore(value: 0)

    Task {
        do {
            let instructions = chat.system ?? ""
            let session = LanguageModelSession(instructions: instructions)

            // Replay conversation history to build session context.
            // For each user message (except the last), call respond() and discard
            // the response since we already have the assistant's reply.
            // Assistant messages between user messages are implicit in the session state.
            var lastResponse = ""
            for (index, msg) in chat.messages.enumerated() {
                if msg.role == "user" {
                    let response = try await session.respond(to: msg.content)
                    lastResponse = response.content
                    // For non-final user messages, we just build up context.
                    // The session internally tracks the conversation.
                }
                // Assistant messages are skipped — the session already generated
                // its own responses which serve as context for subsequent turns.
            }

            box.value = jsonOk(lastResponse)
        } catch let error as LanguageModelSession.GenerationError {
            box.value = generationErrorJSON(error)
        } catch {
            box.value = jsonErr("unknown", error.localizedDescription)
        }
        semaphore.signal()
    }

    semaphore.wait()
    return SRString(box.value)
    #else
    return SRString(jsonErr("unsupported_sdk", "FoundationModels framework not available"))
    #endif
}

// MARK: - Structured (guided) generation

#if canImport(FoundationModels)
/// 整数キーを取り出す。キー不在は nil、present だが Int でない場合は invalid_schema として弾く。
private func intOrNil(_ value: Any?, _ key: String, _ name: String) throws -> Int? {
    guard let value else { return nil }
    guard let intValue = value as? Int else {
        throw NSError(
            domain: "rintel.schema", code: 6,
            userInfo: [NSLocalizedDescriptionKey: "malformed '\(key)' at \(name)"])
    }
    return intValue
}

/// 文字列キーを取り出す。キー不在は nil、present だが String でない場合は invalid_schema として弾く
/// （intOrNil と同じく、サイレントに値を捨てない）。
private func stringOrNil(_ value: Any?, _ key: String, _ name: String) throws -> String? {
    guard let value else { return nil }
    guard let stringValue = value as? String else {
        throw NSError(
            domain: "rintel.schema", code: 6,
            userInfo: [NSLocalizedDescriptionKey: "malformed '\(key)' at \(name)"])
    }
    return stringValue
}

/// その型の buildDynamicSchema が実際に解釈・enforcement する構造キーワード。
///
/// 型ごとに分けるのが肝心。和集合で許可すると `{"type":"string","minItems":3}` のように
/// 別の型に属するキーが通過して黙って無視され、非適合 JSON を「適合」として返しかねない。
private func recognizedSchemaKeys(for type: String) -> Set<String> {
    switch type {
    case "object": return ["type", "description", "properties", "required"]
    case "array": return ["type", "description", "items", "minItems", "maxItems"]
    default: return ["type", "description"]  // string / integer / number / boolean
    }
}

/// 制約を持たない annotation キーワード。無視しても適合性に影響しないため許可する。
/// additionalProperties は閉じたオブジェクト（= 宣言済みプロパティのみ）が生成器の構造上
/// 既に満たされるため、ここに含めて無視する（多くのスキーマ生成器が常時付与するため）。
private let allowedAnnotationKeys: Set<String> = [
    "title", "$schema", "$id", "$comment",
    "default", "examples", "readOnly", "writeOnly", "deprecated",
    "additionalProperties",
]

/// その型で解釈も enforcement もできないキーワードが含まれていれば invalid_schema として弾く。
///
/// denylist ではなく allowlist 方式: 型ごとの認識キーと無害な annotation 以外はすべて拒否する。
/// enum/minimum/pattern だけでなく if/then/else・dependentRequired・patternProperties や
/// 型違いの構造キー（string の minItems 等）も、JSON Schema のキーワードを列挙し続けなくても
/// 恒久的に取りこぼさない（素通りさせると非適合 JSON を「適合」として返しかねないため）。
private func rejectUnsupportedSchemaConstraints(_ node: [String: Any], _ type: String, _ name: String) throws {
    let unknown = Set(node.keys)
        .subtracting(recognizedSchemaKeys(for: type))
        .subtracting(allowedAnnotationKeys)
    if let key = unknown.sorted().first {
        throw NSError(
            domain: "rintel.schema", code: 7,
            userInfo: [NSLocalizedDescriptionKey: "unsupported '\(key)' at \(name)"])
    }
}

/// JSON Schema (subset) を DynamicGenerationSchema に変換する。
///
/// 対応する型: object (properties/required), array (items/minItems/maxItems),
/// string, integer, number, boolean。description は object ノード自身と各プロパティに付与される。
/// enforcement できない制約キーワード（enum/minimum/pattern/if など）は allowlist 方式で
/// 黙って無視せず invalid_schema として拒否する（rejectUnsupportedSchemaConstraints 参照）。
@available(macOS 26.0, *)
private func buildDynamicSchema(_ node: [String: Any], _ name: String) throws -> DynamicGenerationSchema {
    guard let type = node["type"] as? String else {
        let detail = node["type"] == nil ? "missing 'type'" : "malformed 'type'"
        throw NSError(
            domain: "rintel.schema", code: 1,
            userInfo: [NSLocalizedDescriptionKey: "\(detail) at \(name)"])
    }
    try rejectUnsupportedSchemaConstraints(node, type, name)
    switch type {
    case "object":
        // present-but-wrong-type は invalid_schema として弾く。キー不在のときだけ既定値に倒す
        // （サイレントに全プロパティを落とすと空オブジェクト生成になり原因が分かりづらいため）。
        let props: [String: [String: Any]]
        if let raw = node["properties"] {
            guard let typed = raw as? [String: [String: Any]] else {
                throw NSError(
                    domain: "rintel.schema", code: 4,
                    userInfo: [NSLocalizedDescriptionKey: "malformed 'properties' at \(name)"])
            }
            props = typed
        } else {
            props = [:]
        }
        let required: [String]
        if let raw = node["required"] {
            guard let typed = raw as? [String] else {
                throw NSError(
                    domain: "rintel.schema", code: 5,
                    userInfo: [NSLocalizedDescriptionKey: "malformed 'required' at \(name)"])
            }
            required = typed
        } else {
            required = []
        }
        // required に挙げたが properties に無いキーは、サイレントに optional 化されて
        // 必須フィールド欠落を「適合」と扱ってしまうため拒否する（typo 検出）。
        let missingRequired = Set(required).subtracting(props.keys)
        if let key = missingRequired.sorted().first {
            throw NSError(
                domain: "rintel.schema", code: 8,
                userInfo: [NSLocalizedDescriptionKey:
                    "required property '\(key)' missing from 'properties' at \(name)"])
        }
        var properties: [DynamicGenerationSchema.Property] = []
        for (key, child) in props {
            let childSchema = try buildDynamicSchema(child, "\(name)_\(key)")
            properties.append(
                DynamicGenerationSchema.Property(
                    name: key,
                    description: try stringOrNil(child["description"], "description", "\(name)_\(key)"),
                    schema: childSchema,
                    isOptional: !required.contains(key)
                )
            )
        }
        return DynamicGenerationSchema(
            name: name,
            description: try stringOrNil(node["description"], "description", name),
            properties: properties
        )
    case "array":
        guard let items = node["items"] as? [String: Any] else {
            let detail = node["items"] == nil ? "array missing 'items'" : "malformed 'items'"
            throw NSError(
                domain: "rintel.schema", code: 2,
                userInfo: [NSLocalizedDescriptionKey: "\(detail) at \(name)"])
        }
        let itemSchema = try buildDynamicSchema(items, "\(name)_item")
        return DynamicGenerationSchema(
            arrayOf: itemSchema,
            minimumElements: try intOrNil(node["minItems"], "minItems", name),
            maximumElements: try intOrNil(node["maxItems"], "maxItems", name)
        )
    case "string":
        return DynamicGenerationSchema(type: String.self)
    case "integer":
        return DynamicGenerationSchema(type: Int.self)
    case "number":
        return DynamicGenerationSchema(type: Double.self)
    case "boolean":
        return DynamicGenerationSchema(type: Bool.self)
    default:
        throw NSError(
            domain: "rintel.schema", code: 3,
            userInfo: [NSLocalizedDescriptionKey: "unsupported type '\(type)' at \(name)"])
    }
}
#endif

/// 与えられた JSON Schema に従って構造化生成を行う（ブロッキング、シングルターン）。
///
/// 出力は schema に適合する JSON 文字列で、`{"ok": "<json>"}` に包んで返す。
/// 小型モデルでもスキーマ準拠を強制するため、フリーテキスト生成の
/// 不正 JSON・例文丸写しを防げる（生成自体が decodingFailure で失敗する余地は残る）。
@_cdecl("ai_generate_structured")
public func aiGenerateStructured(system: SRString, user: SRString, schema: SRString) -> SRString {
    #if canImport(FoundationModels)
    guard #available(macOS 26.0, *) else {
        return SRString(jsonErr("unsupported_os", "macOS 26.0 or later is required"))
    }

    let systemPrompt = system.toString()
    let userPrompt = user.toString()
    let schemaStr = schema.toString()

    let generationSchema: GenerationSchema
    do {
        guard let data = schemaStr.data(using: .utf8),
              let node = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return SRString(jsonErr("invalid_schema", "Failed to parse JSON schema"))
        }
        let root = try buildDynamicSchema(node, "Root")
        generationSchema = try GenerationSchema(root: root, dependencies: [])
    } catch {
        return SRString(jsonErr("invalid_schema", error.localizedDescription))
    }

    let box = ResultBox(jsonErr("unknown", "Generation did not complete"))
    let semaphore = DispatchSemaphore(value: 0)

    Task {
        do {
            let session = LanguageModelSession(instructions: systemPrompt)
            let response = try await session.respond(to: userPrompt, schema: generationSchema)
            box.value = jsonOk(response.content.jsonString)
        } catch let error as LanguageModelSession.GenerationError {
            box.value = generationErrorJSON(error)
        } catch {
            box.value = jsonErr("unknown", error.localizedDescription)
        }
        semaphore.signal()
    }

    semaphore.wait()
    return SRString(box.value)
    #else
    return SRString(jsonErr("unsupported_sdk", "FoundationModels framework not available"))
    #endif
}
