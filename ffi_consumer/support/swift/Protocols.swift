import Foundation

// MARK: - FFI Protocols
protocol FFIData {
    associatedtype Value

    static var defaultValue: Value { get }
}

/// Describes the structure of all `FFIArray*` types, relying on `Value` for the size of their
/// pointer. `FFIArray*` types need to implement this.
protocol FFIArray: FFIData {
    var ptr: UnsafePointer<Value>! { get }
    var len: UInt { get }
    var cap: UInt { get }

    static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self

    static func free(_ array: Self)
}

/// Describes the structure of all `Option*` types, relying on `Value` for their non-null value
/// type. `Option*` types need to implement this.
protocol FFIOption: FFIData {
    var has_value: Bool { get }
    var value: Value { get }

    static func from(has_value: Bool, value: Value) -> Self

    static func free(_ option: Self)
}

// MARK: - Native protocols
protocol NativeData {
    associatedtype ForeignType

    static var defaultValue: Self { get }

    // Base type
    func toRust() -> ForeignType
    static func fromRust(_ foreignObject: ForeignType) -> Self
}

protocol NativeArrayData: NativeData {
    associatedtype FFIArrayType: FFIArray
}

extension NativeArrayData where FFIArrayType.Value == ForeignType {
    static func ffiArrayInit<T: Collection>(_ collection: T) -> FFIArrayType where T.Element == Self {
        let ffiArray = collection.map { $0.toRust() }
        let len = ffiArray.count
        return ffiArray.withUnsafeBufferPointer { FFIArrayType.from(ptr: $0.baseAddress, len: len) }
    }

    static func ffiArrayFree(_ foreignObject: FFIArrayType) {
        FFIArrayType.free(foreignObject)
    }
}

protocol NativeOptionData: NativeData {
    associatedtype FFIOptionType: FFIOption
}

extension NativeOptionData where FFIOptionType.Value == ForeignType {
    static func optionInit(_ option: Self?) -> FFIOptionType {
        switch option {
        case let .some(wrapped):
            return FFIOptionType.from(has_value: true, value: wrapped.toRust())
        case .none:
            return FFIOptionType.from(has_value: false, value: FFIOptionType.defaultValue)
        }
    }
}

/// Support for optional raw types, like numeric primitives.
extension Optional where Wrapped: NativeOptionData, Wrapped.FFIOptionType.Value == Wrapped.ForeignType {
    func toRust() -> Wrapped.FFIOptionType {
        Wrapped.optionInit(self)
    }

    static func fromRust(_ foreignObject: Wrapped.FFIOptionType) -> Self {
        guard foreignObject.has_value else {
            return none
        }
        let option = Wrapped.fromRust(foreignObject.value)
        Wrapped.FFIOptionType.free(foreignObject)
        return option
    }
}

/// Support for optional boxed types, like a pointer to a `BoundaryMap` that may be nil.
extension Optional: FFIData where Wrapped == OpaquePointer {
    static var defaultValue: OpaquePointer? { return nil }
}

/// Support for optional boxed types, like a pointer to a `BoundaryMap` that may be nil. This is
/// a bit of an odd conformance; we don't actually manage any memory through
/// `Optional<OpaquePointer>`. It's really just a convenience for pushing optional pointers through
/// the same extensions as other optional types.
extension Optional: FFIOption where Wrapped == OpaquePointer {
    /// The FFI value type is an `OpaquePointer?`, which is to say that this native type and its FFI
    /// type are the same (an optional pointer).
    typealias Value = OpaquePointer?

    var has_value: Bool {
        switch self {
        case .some(_):
            return true
        case .none:
            return false
        }
    }

    var value: Value {
        guard case let .some(wrapped) = self else {
            return nil
        }
        return wrapped
    }

    static func from(has_value: Bool, value: Value) -> Self {
        guard has_value else { return .none }
        return .some(value!)
    }

    static func free(_ option: Self) {
        // Nothing to do here; we don't alloc or free any FFI memory for `Optional<OpaquePointer>`;
        // that's managed by the wrapping consumer object that owns this pointer. When we initialize
        // an `Address` in Rust, we store an `OpaquePointer` to it in a Swift `Address`, which has a
        // call to free that pointer's memory when the Swift instance is deallocated. (And if this
        // is a `.none`, then there's really nothing to do here.)
    }
}

/// This lets us do `[NativeFoo]?.fromRust(instanceOfFFIArrayFooThatMightBeNil)` and 
/// `[instanceOfNativeFooThatMightBeNil]?.toRust()` whenever `NativeFoo` is `FFIArray` and
/// `FFIArrayFoo` is `FFIArray` (both of which are trivial to generate for pretty much any type).
extension Optional where
    Wrapped: Collection,
    Wrapped.Element: NativeArrayData,
    Wrapped.Element.FFIArrayType.Value == Wrapped.Element.ForeignType
{
    func toRust() -> Wrapped.Element.FFIArrayType {
        switch self {
        case let .some(wrapped):
            return wrapped.toRust()
        case .none:
            return Wrapped.Element.FFIArrayType.from(ptr: nil, len: 0)
        }
    }

    static func fromRust(_ foreignObject: Wrapped.Element.FFIArrayType) -> [Wrapped.Element]? {
        guard foreignObject.ptr != nil else { return .none }
        return Wrapped.fromRust(foreignObject)
    }
}

/// This lets us do `[NativeFoo].fromRust(instanceOfFFIArrayFoo)` and 
/// `[instanceOfNativeFoo].toRust()` whenever `NativeFoo` is `FFIArray` and `FFIArrayFoo` is
/// `FFIArray` (both of which are trivial to generate for pretty much any type).
extension Collection where Element: NativeArrayData, Element.FFIArrayType.Value == Element.ForeignType {
    func toRust() -> Element.FFIArrayType {
        Element.ffiArrayInit(self)
    }

    static func fromRust(_ foreignObject: Element.FFIArrayType) -> [Element] {
        let count = Int(foreignObject.len)
        var nativeArray = [Element]()
        for i in 0..<count {
            nativeArray.append(Element.fromRust(foreignObject.ptr[i]))
        }
        Element.ffiArrayFree(foreignObject)
        return nativeArray
    }
}
