#![allow(dead_code)] // Complete API module, not all methods currently used
//! Heap implementation for the interpreter
//!
//! This module provides heap memory management with:
//! - Explicit allocation/deallocation (malloc/free)
//! - Tombstone tracking for freed blocks (enables reverse execution)
//! - Per-byte initialization tracking
//! - Use-after-free and double-free detection
//!
//! # Error Handling
//!
//! Methods return `Result<_, String>` for errors. While a custom error type would be
//! more idiomatic, this is an internal API and the string errors are converted to
//! `RuntimeError` at the interpreter boundary. Refactoring to a custom type would
//! require changes to 50+ call sites with minimal functional benefit.

use super::value::Address;
use crate::interpreter::constants::HEAP_ADDRESS_START;

/// State of a heap block
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockState {
    Allocated,
    Tombstone, // Freed but kept for reverse execution
}

/// A block of heap memory
#[derive(Debug, Clone)]
pub struct HeapBlock {
    pub data: Vec<u8>, // Raw bytes
    pub size: usize,
    pub state: BlockState,
    pub init_map: Vec<bool>, // Per-byte initialization tracking
}

impl HeapBlock {
    pub fn new(size: usize) -> Self {
        HeapBlock {
            data: vec![0; size],
            size,
            state: BlockState::Allocated,
            init_map: vec![false; size],
        }
    }

    /// Check if a byte range is initialized
    pub fn is_initialized(&self, offset: usize, size: usize) -> bool {
        if offset + size > self.size {
            return false;
        }
        self.init_map[offset..offset + size].iter().all(|&b| b)
    }

    /// Mark a byte range as initialized
    pub fn mark_initialized(&mut self, offset: usize, size: usize) {
        if offset + size <= self.size {
            for i in offset..offset + size {
                self.init_map[i] = true;
            }
        }
    }

    /// Mark a byte range as uninitialized
    pub fn mark_uninitialized(&mut self, offset: usize, size: usize) {
        if offset + size <= self.size {
            for i in offset..offset + size {
                self.init_map[i] = false;
            }
        }
    }

    /// Read bytes from the block
    pub fn read_bytes(&self, offset: usize, size: usize) -> Option<&[u8]> {
        if offset + size <= self.size {
            Some(&self.data[offset..offset + size])
        } else {
            None
        }
    }

    /// Write bytes to the block
    pub fn write_bytes(&mut self, offset: usize, bytes: &[u8]) -> Result<(), String> {
        if offset + bytes.len() > self.size {
            return Err(format!(
                "Buffer overrun: attempted to write {} bytes at offset {} in block of size {}",
                bytes.len(),
                offset,
                self.size
            ));
        }
        self.data[offset..offset + bytes.len()].copy_from_slice(bytes);
        self.mark_initialized(offset, bytes.len());
        Ok(())
    }
}

/// The heap
#[derive(Debug, Clone)]
pub struct Heap {
    allocations: std::collections::HashMap<Address, HeapBlock>,
    next_address: Address,
    total_allocated_bytes: usize,
    max_heap_size: usize,
}

impl Heap {
    /// Create a new heap with a maximum size limit
    pub fn new(max_heap_size: usize) -> Self {
        Heap {
            allocations: std::collections::HashMap::new(),
            next_address: HEAP_ADDRESS_START, // Start heap at high address
            total_allocated_bytes: 0,
            max_heap_size,
        }
    }

    /// Allocate a block of memory
    pub fn allocate(&mut self, size: usize) -> Result<Address, String> {
        if self.total_allocated_bytes + size > self.max_heap_size {
            return Err(format!(
                "Out of memory: requested {} bytes, {} already allocated, limit is {}",
                size, self.total_allocated_bytes, self.max_heap_size
            ));
        }

        let addr = self.next_address;
        self.next_address += size as u64;
        self.allocations.insert(addr, HeapBlock::new(size));
        self.total_allocated_bytes += size;

        Ok(addr)
    }

    /// Free a block of memory (mark as tombstone)
    pub fn free(&mut self, addr: Address) -> Result<(), String> {
        match self.allocations.get_mut(&addr) {
            Some(block) if block.state == BlockState::Allocated => {
                block.state = BlockState::Tombstone;
                Ok(())
            }
            Some(block) if block.state == BlockState::Tombstone => {
                Err(format!("Double free detected at address 0x{:x}", addr))
            }
            None => Err(format!(
                "Invalid free: address 0x{:x} was never allocated",
                addr
            )),
            _ => unreachable!(),
        }
    }

    /// Get a heap block (returns error if tombstone or doesn't exist)
    pub fn get_block(&self, addr: Address) -> Result<&HeapBlock, String> {
        match self.allocations.get(&addr) {
            Some(block) if block.state == BlockState::Allocated => Ok(block),
            Some(_) => Err(format!(
                "Use-after-free: address 0x{:x} has been freed",
                addr
            )),
            None => Err(format!(
                "Invalid pointer: address 0x{:x} not allocated",
                addr
            )),
        }
    }

    /// Get a mutable heap block
    pub fn get_block_mut(&mut self, addr: Address) -> Result<&mut HeapBlock, String> {
        match self.allocations.get_mut(&addr) {
            Some(block) if block.state == BlockState::Allocated => Ok(block),
            Some(_) => Err(format!(
                "Use-after-free: address 0x{:x} has been freed",
                addr
            )),
            None => Err(format!(
                "Invalid pointer: address 0x{:x} not allocated",
                addr
            )),
        }
    }

    /// Get all allocations (for UI display, includes tombstones)
    pub fn allocations(&self) -> &std::collections::HashMap<Address, HeapBlock> {
        &self.allocations
    }

    /// Get total allocated bytes
    pub fn total_allocated(&self) -> usize {
        self.total_allocated_bytes
    }

    /// Get max heap size
    pub fn max_size(&self) -> usize {
        self.max_heap_size
    }

    /// Write a single byte to an address
    pub fn write_byte(&mut self, addr: Address, byte: u8) -> Result<(), String> {
        // Find the block containing this address
        let mut target_block_addr = None;
        for (&block_addr, block) in &self.allocations {
            if addr >= block_addr && addr < block_addr + block.size as u64 {
                target_block_addr = Some(block_addr);
                break;
            }
        }

        let block_addr = target_block_addr.ok_or_else(|| {
            format!(
                "Invalid write: address 0x{:x} not in any allocated block",
                addr
            )
        })?;

        let block = self.get_block_mut(block_addr)?;
        let offset = (addr - block_addr) as usize;
        block.data[offset] = byte;
        block.init_map[offset] = true;
        Ok(())
    }

    /// Read a single byte from an address
    pub fn read_byte(&self, addr: Address) -> Result<u8, String> {
        // Find the block containing this address
        let mut target_block_addr = None;
        for (&block_addr, block) in &self.allocations {
            if addr >= block_addr && addr < block_addr + block.size as u64 {
                target_block_addr = Some(block_addr);
                break;
            }
        }

        let block_addr = target_block_addr.ok_or_else(|| {
            format!(
                "Invalid read: address 0x{:x} not in any allocated block",
                addr
            )
        })?;

        let block = self.get_block(block_addr)?;
        let offset = (addr - block_addr) as usize;

        if !block.init_map[offset] {
            return Err(format!("Uninitialized read at address 0x{:x}", addr));
        }

        Ok(block.data[offset])
    }

    /// Write multiple bytes starting at an address
    pub fn write_bytes_at(&mut self, addr: Address, bytes: &[u8]) -> Result<(), String> {
        for (i, &byte) in bytes.iter().enumerate() {
            self.write_byte(addr + i as u64, byte)?;
        }
        Ok(())
    }

    /// Read multiple bytes starting at an address
    pub fn read_bytes_at(&self, addr: Address, size: usize) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::with_capacity(size);
        for i in 0..size {
            bytes.push(self.read_byte(addr + i as u64)?);
        }
        Ok(bytes)
    }
}

impl Default for Heap {
    fn default() -> Self {
        // Default heap size: 10 MB
        Self::new(10 * 1024 * 1024)
    }
}
