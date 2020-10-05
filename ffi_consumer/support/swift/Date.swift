extension FFIArrayTimeStamp: FFIArray {
    typealias Value = TimeStamp

    static var defaultValue: Value { TimeStamp(secs: 0, nsecs: 0) }

    static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self {
        ffi_array_time_stamp_init(ptr, len)
    }

    static func free(_ array: Self) {
        free_ffi_array_time_stamp(array)
    }
}

extension OptionTimeStamp: FFIOption {
    typealias Value = TimeStamp
    static var defaultValue: Value { TimeStamp(secs: 0, nsecs: 0) }

    static func from(has_value: Bool, value: TimeStamp) -> OptionTimeStamp {
        option_time_stamp_init(has_value, value)
    }

    static func free(_ option: OptionTimeStamp) {
        free_option_time_stamp(option)
    }
}

extension Date: NativeData {
    typealias ForeignType = TimeStamp

    static var defaultValue: Self { Self() }

    private static let nsecs_per_sec: Double = 1_000_000_000

    func toRust() -> ForeignType {
        let (seconds, subSeconds) = modf(timeIntervalSince1970)
        return ForeignType(secs: Int64(seconds), nsecs: UInt32(subSeconds * Self.nsecs_per_sec))
    }

    static func fromRust(_ foreignObject: ForeignType) -> Self {
        let interval = Double(foreignObject.secs) + Double(Double(foreignObject.nsecs) / Self.nsecs_per_sec)
        return Date(timeIntervalSince1970: interval)
    }
}

extension Date: NativeArrayData {
    typealias FFIArrayType = FFIArrayTimeStamp
}

extension Date: NativeOptionData {
    typealias FFIOptionType = OptionTimeStamp
}
