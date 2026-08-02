import Foundation
import Security
import SwiftUI

// API keys are stored in the Keychain rather than UserDefaults. A UserDefaults plist sits
// unencrypted in the user's Library and is readable by any process running as that user,
// which is a poor home for billable third-party credentials.
enum KeychainStore {
    private static let service = "io.artificialhumanity.council-of-experts"

    static func read(_ account: String) -> String {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]

        var item: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess,
              let data = item as? Data,
              let value = String(data: data, encoding: .utf8)
        else {
            return ""
        }
        return value
    }

    static func write(_ value: String, account: String) {
        let identity: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]

        // Clearing a field should remove the stored secret, not save an empty one.
        guard !value.isEmpty else {
            SecItemDelete(identity as CFDictionary)
            return
        }

        let data = Data(value.utf8)
        let status = SecItemUpdate(
            identity as CFDictionary,
            [kSecValueData as String: data] as CFDictionary
        )

        if status == errSecItemNotFound {
            var insert = identity
            insert[kSecValueData as String] = data
            insert[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
            SecItemAdd(insert as CFDictionary, nil)
        }
    }

    // Moves a key written by an earlier build out of UserDefaults, so upgrading users keep
    // working without re-entering credentials and the plaintext copy doesn't linger.
    static func migrateFromUserDefaults(_ account: String) {
        guard read(account).isEmpty,
              let legacy = UserDefaults.standard.string(forKey: account),
              !legacy.isEmpty
        else {
            UserDefaults.standard.removeObject(forKey: account)
            return
        }

        write(legacy, account: account)
        UserDefaults.standard.removeObject(forKey: account)
    }
}

final class CredentialStore: ObservableObject {
    static let accounts = ["openAiKey", "anthropicKey", "geminiKey", "grokKey"]

    @Published var openAiKey: String { didSet { KeychainStore.write(openAiKey, account: "openAiKey") } }
    @Published var anthropicKey: String { didSet { KeychainStore.write(anthropicKey, account: "anthropicKey") } }
    @Published var geminiKey: String { didSet { KeychainStore.write(geminiKey, account: "geminiKey") } }
    @Published var grokKey: String { didSet { KeychainStore.write(grokKey, account: "grokKey") } }

    init() {
        CredentialStore.accounts.forEach(KeychainStore.migrateFromUserDefaults)

        openAiKey = KeychainStore.read("openAiKey")
        anthropicKey = KeychainStore.read("anthropicKey")
        geminiKey = KeychainStore.read("geminiKey")
        grokKey = KeychainStore.read("grokKey")
    }
}
