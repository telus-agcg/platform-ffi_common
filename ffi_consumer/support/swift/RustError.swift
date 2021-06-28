import Foundation

public struct RustError: Error {
    private static let unknownError = NSLocalizedString(
        "Unknown error",
        bundle: Bundle(for: BundleID.self),
        comment: "An unknown error occurred")

    public let errorMessage: String

    private init(errorMessage: String) {
        self.errorMessage = errorMessage
    }

    public static func getLastError() -> Self {
        Self(
            errorMessage: get_last_err_msg()
                .map { String.fromRust($0) }
                ?? unknownError
        )
    }
}

public func handle<T: NativeData>(result: T.ForeignType?) -> Result<T, RustError> {
    guard let result = result else {
        return .failure(RustError.getLastError())
    }
    return .success(T.fromRust(result))
}
