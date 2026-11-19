//! l23a tape lowering.

#[cfg(test)]
mod tests {
    use loft_abi::Tier0Opcode;

    // The VM lane adds per-op evaluation parity against the twin here.
    #[test]
    fn operand_limits() {
        let op = Tier0Opcode::Ret;
        assert_eq!(op.pack(31, 31, 31, 2047), Some(0xdfff_ffff));
        for (dst, a, b, imm) in [(32, 0, 0, 0), (0, 32, 0, 0), (0, 0, 32, 0), (0, 0, 0, 2048)] {
            assert_eq!(op.pack(dst, a, b, imm), None);
        }
    }
}
