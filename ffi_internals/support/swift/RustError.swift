import Foundation

// MARK: - RustError
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

// MARK: - LocalizedError
extension RustError: LocalizedError {
    public var errorDescription: String? { errorMessage }
}

// MARK: - Result handlers
public func handle<T: NativeData>(
    result: T.ForeignType
) -> Result<T, RustError> where T.ForeignType == Optional<OpaquePointer> {
    guard let result = result else {
        return .failure(RustError.getLastError())
    }
    return .success(T.fromRust(result))
}

public func handle<T: NativeArrayData>(
    result: T.FFIArrayType
) -> Result<[T], RustError> where T.ForeignType == T.FFIArrayType.Value {
    guard result.ptr != nil else {
        return .failure(RustError.getLastError())
    }
    return .success([T].fromRust(result))
}
