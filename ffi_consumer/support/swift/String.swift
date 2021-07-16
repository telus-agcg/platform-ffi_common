// `String` does not conform to the `NativeData` protocols, because we need to manage its memory
// differently from other types. Normal reference types (i.e., complex objects like an
// `Organization`) are initialized with a reference to the Rust object that is **NOT** dropped on
// init; instead, we tell Rust we're done when the Swift object is deinitialized. When we retrieve a
// string from Rust, we want a native type like normal, but we also want to tell Rust to clean its
// side up (because we can't tell it that during deinitialization, since we don't know whether a
// particular `String` came from Swift or Rust). Normally that's fine, but when we're intializing an
// `Array` on the Swift side, we always tell Rust that we're done with it immediately afterward 
// (same reason as above with `String`), which will tell Rust to deallocate the memory for that
// `Vec`, **including the memory for each element in Vec**, which will result in a double free error
// if we've already freed each element's memory when initializing a Swift `String` for the Swift
// `Array`. So, simple solution is to handle Swift `[String].fromRust()`` differently.

extension FFIArrayString: FFIArray {
    public typealias Value = UnsafePointer<CChar>?

    public static func from(ptr: UnsafePointer<Value>?, len: Int) -> FFIArrayString {
        ffi_array_string_init(ptr, len)
    }

    public static func free(_ array: FFIArrayString) {
        ffi_array_string_free(array)
    }
}

public extension String {
    func toRust() -> UnsafePointer<CChar>? {
        (self as NSString).utf8String
    }

    static func fromRust(_ foreignObject: UnsafePointer<CChar>?) -> String {
        let string = String(cString: foreignObject!)
        free_rust_string(foreignObject)
        return string
    }
}

public extension Array where Element == String {
    func toRust() -> FFIArrayString {
        let ffiArray = map { $0.toRust() }
        let len = ffiArray.count
        return ffiArray.withUnsafeBufferPointer { FFIArrayString.from(ptr: $0.baseAddress, len: len) }
    }

    static func fromRust(_ foreignObject: FFIArrayString) -> Self {
        let count = Int(foreignObject.len)
        var nativeArray = Self(repeating: "", count: count)
        for i in 0..<count {
            nativeArray[i] = String(cString: foreignObject.ptr[i]!)
        }
        ffi_array_string_free(foreignObject)
        return nativeArray
    }
}

public extension Optional where Wrapped == String {
    static func fromRust(_ foreignObject: UnsafePointer<CChar>?) -> Self {
        guard let foreignObject = foreignObject else {
            return nil
        }
        return String.fromRust(foreignObject)
    }

    func toRust() -> UnsafePointer<CChar>? {
        guard case let .some(value) = self else {
            return nil
        }
        return value.toRust()
    }
}

public extension Optional where Wrapped == [String] {
    static func from(ptr: UnsafePointer<UnsafePointer<CChar>?>, len: Int) -> FFIArrayString {
        ffi_array_string_init(ptr, len)
    }

    static func free(_ array: FFIArrayString) {
        ffi_array_string_free(array)
    }

    static func fromRust(_ foreignObject: FFIArrayString) -> Self {
        guard foreignObject.ptr != nil else {
            return none
        }
        return Wrapped.fromRust(foreignObject)
    }

    func toRust() -> FFIArrayString {
        switch self {
        case let .some(wrapped):
            let ffiArray = wrapped.map { $0.toRust() }
            let len = ffiArray.count
            return ffiArray.withUnsafeBufferPointer { ffi_array_string_init($0.baseAddress, len) }
        case .none:
            return ffi_array_string_init(nil, 0)
        }
    }
}
