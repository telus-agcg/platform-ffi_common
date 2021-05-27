import Foundation

// MARK: - FFI Protocols
public protocol FFIData {
    associatedtype Value
}

/// Describes the structure of all `FFIArray*` types, relying on `Value` for the size of their
/// pointer. `FFIArray*` types need to implement this.
public protocol FFIArray: FFIData {
    var ptr: UnsafePointer<Value>! { get }
    var len: UInt { get }
    var cap: UInt { get }

    static func from(ptr: UnsafePointer<Value>?, len: Int) -> Self

    static func free(_ array: Self)
}

// MARK: - Native protocols
public protocol NativeData {
    associatedtype ForeignType

    // Base type
    func toRust() -> ForeignType
    static func fromRust(_ foreignObject: ForeignType) -> Self
}

public protocol NativeArrayData: NativeData {
    associatedtype FFIArrayType: FFIArray
}

public extension NativeArrayData where FFIArrayType.Value == ForeignType {
    static func ffiArrayInit<T: Collection>(_ collection: T) -> FFIArrayType where T.Element == Self {
        let ffiArray = collection.map { $0.toRust() }
        let len = ffiArray.count
        return ffiArray.withUnsafeBufferPointer { FFIArrayType.from(ptr: $0.baseAddress, len: len) }
    }

    static func ffiArrayFree(_ foreignObject: FFIArrayType) {
        FFIArrayType.free(foreignObject)
    }
}

/// This lets us do `[NativeFoo]?.fromRust(instanceOfFFIArrayFooThatMightBeNil)` and 
/// `[instanceOfNativeFooThatMightBeNil]?.toRust()` whenever `NativeFoo` is `FFIArray` and
/// `FFIArrayFoo` is `FFIArray` (both of which are trivial to generate for pretty much any type).
public extension Optional where
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
public extension Collection where Element: NativeArrayData, Element.FFIArrayType.Value == Element.ForeignType {
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
