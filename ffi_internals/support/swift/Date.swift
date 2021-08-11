extension FFIArrayTimeStamp: FFIArray {
    public typealias Value = OpaquePointer?

    public static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {
        ffi_array_TimeStamp_init(ptr, len)
    }

    public static func free(_ array: Self) {
        ffi_array_TimeStamp_free(array)
    }
}

public extension Optional where Wrapped == Date {
    func clone() -> OpaquePointer? {
        switch self {
        case let .some(value):
            return value.clone()
        case .none:
            return nil
        }
    }

    func borrowReference() -> OpaquePointer? {
        switch self {
        case let .some(value):
            return value.borrowReference()
        case .none:
            return nil
        }
    }

    static func fromRust(_ ptr: OpaquePointer?) -> Self {
        guard let ptr = ptr else {
            return .none
        }
        return Wrapped.fromRust(ptr)
    }
}

extension Date: NativeData {
    public typealias ForeignType = OpaquePointer

    private static let nsecs_per_sec: Double = 1_000_000_000

    public func clone() -> ForeignType {
        let (seconds, subSeconds) = modf(timeIntervalSince1970)
        return time_stamp_init(Int64(seconds), UInt32(subSeconds * Self.nsecs_per_sec))
    }

    public func borrowReference() -> ForeignType {
        let (seconds, subSeconds) = modf(timeIntervalSince1970)
        return time_stamp_init(Int64(seconds), UInt32(subSeconds * Self.nsecs_per_sec))
    }

    public static func fromRust(_ foreignObject: ForeignType) -> Self {
        let secs = get_time_stamp_secs(foreignObject)
        let nsecs = get_time_stamp_nsecs(foreignObject)
        let interval = Double(secs) + Double(Double(nsecs) / Self.nsecs_per_sec)
        let date = Date(timeIntervalSince1970: interval)
        time_stamp_free(foreignObject)
        return date
    }
}

extension Date: NativeArrayData {
    public typealias FFIArrayType = FFIArrayTimeStamp
}
