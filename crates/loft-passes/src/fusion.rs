//! l23b fixed-step lowering.

#[cfg(test)]
mod tests {
    use loft_abi::{AdvanceArgs, advance};

    // The fusion lane extends this to compare live and baked state after regrouping ticks.
    #[test]
    fn advance_contract() {
        assert_eq!(AdvanceArgs::ENTRY, "advance");
        assert_eq!(AdvanceArgs::DT, 1.0 / 120.0);
        assert_eq!(
            advance(60, 124, 1),
            Some(AdvanceArgs {
                from_tick: 60,
                to_tick: 124,
                keep_stride: 1,
            })
        );
        assert!(advance(60, 125, 1).is_none());
        assert!(advance(60, 59, 1).is_none());
        assert!(advance(u32::MAX, u32::MAX, 0).is_some());

        let module = naga::front::wgsl::parse_str(AdvanceArgs::WGSL).unwrap();
        let (_, ty) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("AdvanceArgs"))
            .unwrap();
        let naga::TypeInner::Struct { members, span } = &ty.inner else {
            panic!("expected a struct")
        };
        assert_eq!(*span, 12);
        assert_eq!(members.len(), 3);
        for (field, (name, offset)) in
            members
                .iter()
                .zip([("from_tick", 0), ("to_tick", 4), ("keep_stride", 8)])
        {
            assert_eq!((field.name.as_deref(), field.offset), (Some(name), offset));
            assert_eq!(
                module.types[field.ty].inner,
                naga::TypeInner::Scalar(naga::Scalar::U32)
            );
        }
    }
}
