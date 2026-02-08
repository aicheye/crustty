// Constants for the C interpreter

/// Starting address for heap allocations
/// Heap addresses start at 0x10000000 to clearly distinguish them from stack addresses
pub const HEAP_ADDRESS_START: u64 = 0x1000_0000;

/// Starting address for stack variable addresses
/// Stack addresses start at 0x00000004
pub const STACK_ADDRESS_START: u64 = 0x0000_0004;
