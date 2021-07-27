use class::JavaClassReader;
use opcodes::*;
use opcodes::Opcode::*;

/// Reads in a single opcode from a `JavaClassReader` and returns the read-in Opcode.
/// Should never fail with a well-formed Java 8 class file.
pub fn to_opcode(r: &mut JavaClassReader, method_start: u32) -> Option<Opcode> {
    let bytecode = r.next8().ok()?;
    let ans = match bytecode {
        0x00 => nop,
        0x01 => aconst_null,
        0x02 => iconst_m1,
        0x03 => iconst_0,
        0x04 => iconst_1,
        0x05 => iconst_2,
        0x06 => iconst_3,
        0x07 => iconst_4,
        0x08 => iconst_5,
        0x09 => lconst_0,
        0x0a => lconst_1,
        0x0b => fconst_0,
        0x0c => fconst_1,
        0x0d => fconst_2,
        0x0e => dconst_0,
        0x0f => dconst_1,
        0x10 => bipush { val: r.next8().ok()? },
        0x11 => sipush { val: r.next16().ok()? },
        0x12 => ldc { index: r.next8().ok()? },
        0x13 => ldc_w { index: r.next16().ok()? },
        0x14 => ldc2_w { index: r.next16().ok()? },
        0x15 => iload { index: r.next8().ok()? },
        0x16 => lload { index: r.next8().ok()? },
        0x17 => fload { index: r.next8().ok()? },
        0x18 => dload { index: r.next8().ok()? },
        0x19 => aload { index: r.next8().ok()? },
        0x1a => iload_0,
        0x1b => iload_1,
        0x1c => iload_2,
        0x1d => iload_3,
        0x1e => lload_0,
        0x1f => lload_1,
        0x20 => lload_2,
        0x21 => lload_3,
        0x22 => fload_0,
        0x23 => fload_1,
        0x24 => fload_2,
        0x25 => fload_3,
        0x26 => dload_0,
        0x27 => dload_1,
        0x28 => dload_2,
        0x29 => dload_3,
        0x2a => aload_0,
        0x2b => aload_1,
        0x2c => aload_2,
        0x2d => aload_3,
        0x2e => iaload,
        0x2f => laload,
        0x30 => faload,
        0x31 => daload,
        0x32 => aaload,
        0x33 => baload,
        0x34 => caload,
        0x35 => saload,
        0x36 => istore { index: r.next8().ok()? },
        0x37 => lstore { index: r.next8().ok()? },
        0x38 => fstore { index: r.next8().ok()? },
        0x39 => dstore { index: r.next8().ok()? },
        0x3a => astore { index: r.next8().ok()? },
        0x3b => istore_0,
        0x3c => istore_1,
        0x3d => istore_2,
        0x3e => istore_3,
        0x3f => lstore_0,
        0x40 => lstore_1,
        0x41 => lstore_2,
        0x42 => lstore_3,
        0x43 => fstore_0,
        0x44 => fstore_1,
        0x45 => fstore_2,
        0x46 => fstore_3,
        0x47 => dstore_0,
        0x48 => dstore_1,
        0x49 => dstore_2,
        0x4a => dstore_3,
        0x4b => astore_0,
        0x4c => astore_1,
        0x4d => astore_2,
        0x4e => astore_3,
        0x4f => iastore,
        0x50 => lastore,
        0x51 => fastore,
        0x52 => dastore,
        0x53 => aastore,
        0x54 => bastore,
        0x55 => castore,
        0x56 => sastore,
        0x57 => pop,
        0x58 => pop2,
        0x59 => dup,
        0x5a => dup_x1,
        0x5b => dup_x2,
        0x5c => dup2,
        0x5d => dup2_x1,
        0x5e => dup2_x2,
        0x5f => swap,
        0x60 => iadd,
        0x61 => ladd,
        0x62 => fadd,
        0x63 => dadd,
        0x64 => isub,
        0x65 => lsub,
        0x66 => fsub,
        0x67 => dsub,
        0x68 => imul,
        0x69 => lmul,
        0x6a => fmul,
        0x6b => dmul,
        0x6c => idiv,
        0x6d => ldiv,
        0x6e => fdiv,
        0x6f => ddiv,
        0x70 => irem,
        0x71 => lrem,
        0x72 => frem,
        0x73 => drem,
        0x74 => ineg,
        0x75 => lneg,
        0x76 => fneg,
        0x77 => dneg,
        0x78 => ishl,
        0x79 => lshl,
        0x7a => ishr,
        0x7b => lshr,
        0x7c => iushr,
        0x7d => lushr,
        0x7e => iand,
        0x7f => land,
        0x80 => ior,
        0x81 => lor,
        0x82 => ixor,
        0x83 => lxor,
        0x84 => iinc { index: r.next8().ok()?, const_: r.next8().ok()? as i8 },
        0x85 => i2l,
        0x86 => i2f,
        0x87 => i2d,
        0x88 => l2i,
        0x89 => l2f,
        0x8a => l2d,
        0x8b => f2i,
        0x8c => f2l,
        0x8d => f2d,
        0x8e => d2i,
        0x8f => d2l,
        0x90 => d2f,
        0x91 => i2b,
        0x92 => i2c,
        0x93 => i2s,
        0x94 => lcmp,
        0x95 => fcmpl,
        0x96 => fcmpg,
        0x97 => dcmpl,
        0x98 => dcmpg,
        0x99 => ifeq { branch: r.next16().ok()? as i16 },
        0x9a => ifne { branch: r.next16().ok()? as i16 },
        0x9b => iflt { branch: r.next16().ok()? as i16 },
        0x9c => ifge { branch: r.next16().ok()? as i16 },
        0x9d => ifgt { branch: r.next16().ok()? as i16 },
        0x9e => ifle { branch: r.next16().ok()? as i16 },
        0x9f => if_icmpeq { branch: r.next16().ok()? as i16 },
        0xa0 => if_icmpne { branch: r.next16().ok()? as i16 },
        0xa1 => if_icmplt { branch: r.next16().ok()? as i16 },
        0xa2 => if_icmpge { branch: r.next16().ok()? as i16 },
        0xa3 => if_icmpgt { branch: r.next16().ok()? as i16 },
        0xa4 => if_icmple { branch: r.next16().ok()? as i16 },
        0xa5 => if_acmpeq { branch: r.next16().ok()? as i16 },
        0xa6 => if_acmpne { branch: r.next16().ok()? as i16 },
        0xa7 => goto { branch: r.next16().ok()? as i16 },
        0xa8 => jsr { branch: r.next16().ok()? as i16 },
        0xa9 => ret { index: r.next8().ok()? },
        0xaa => {
            let padding = 4 - ((r.dist() - method_start) % 4) as u8;
            let padding = if padding == 4 {0} else {padding};
            for _ in 0..padding { r.next8().ok()?; }
            let default = r.next32().ok()? as i32;
            let low = r.next32().ok()? as i32;
            let high = r.next32().ok()? as i32;
            let npairs = high - low + 1;
            let mut jump_offsets = Vec::new();
            for _ in 0..npairs {
                jump_offsets.push(r.next32().ok()? as i32);
            }
            tableswitch { default, low, high, jump_offsets, padding }
        }
        0xab => {
            let padding = 4 - ((r.dist() - method_start) % 4) as u8;
            let padding = if padding == 4 {0} else {padding};
            for _ in 0..padding { r.next8().ok()?; }
            let default = r.next32().ok()? as i32;
            let npairs = r.next32().ok()? as i32;
            let mut offsets = Vec::new();
            for _ in 0..npairs {
                offsets.push(
                    (r.next32().ok()? as i32,
                     r.next32().ok()? as i32));
            }
            lookupswitch { default, match_offset_pairs: offsets, padding }
        }
        0xac => ireturn,
        0xad => lreturn,
        0xae => freturn,
        0xaf => dreturn,
        0xb0 => areturn,
        0xb1 => return_,
        0xb2 => getstatic { index: r.next16().ok()? },
        0xb3 => putstatic { index: r.next16().ok()? },
        0xb4 => getfield { index: r.next16().ok()? },
        0xb5 => putfield { index: r.next16().ok()? },
        0xb6 => invokevirtual { index: r.next16().ok()? },
        0xb7 => invokespecial { index: r.next16().ok()? },
        0xb8 => invokestatic { index: r.next16().ok()? },
        0xb9 => {
            let ans = invokeinterface { index: r.next16().ok()?, count: r.next8().ok()? };
            r.next8().ok()?;
            ans
        }
        0xba => {
            let ans = invokedynamic { index: r.next16().ok()? };
            r.next8().ok()?;
            r.next8().ok()?;
            ans
        }
        0xbb => new { index: r.next16().ok()? },
        0xbc => newarray { atype: r.next8().ok()? },
        0xbd => anewarray { index: r.next16().ok()? },
        0xbe => arraylength,
        0xbf => athrow,
        0xc0 => checkcast { index: r.next16().ok()? },
        0xc1 => instanceof { index: r.next16().ok()? },
        0xc2 => monitorenter,
        0xc3 => monitorexit,
        0xc4 => {
            let opcode = r.next8().ok()?;
            if opcode == 0x84 {
                wide_iinc {
                    index: r.next16().ok()?,
                    const_: r.next16().ok()? as i16,
                }
            } else {
                wide {
                    opcode,
                    index: r.next16().ok()?,
                }
            }
        }
        0xc5 => multianewarray { index: r.next16().ok()?, dimensions: r.next8().ok()? },
        0xc6 => ifnull { branch: r.next16().ok()? as i16 },
        0xc7 => ifnonnull { branch: r.next16().ok()? as i16 },
        0xc8 => goto_w { branch: r.next32().ok()? as i32 },
        0xc9 => jsr_w { branch: r.next32().ok()? as i32 },
        0xca => breakpoint,
        0xcb..=0xfd => reserved,
        0xfe => impdep1,
        0xff => impdep2,
    };
    Some(ans)
}

/// Converts an `Opcode` into bytes
pub fn to_bytecode(opcode: Opcode) -> (u8, Vec<u8>) {
    let mut args = Vec::new();
    let byte = match opcode {
        nop => 0x00,
        aconst_null => 0x01,
        iconst_m1 => 0x02,
        iconst_0 => 0x03,
        iconst_1 => 0x04,
        iconst_2 => 0x05,
        iconst_3 => 0x06,
        iconst_4 => 0x07,
        iconst_5 => 0x08,
        lconst_0 => 0x09,
        lconst_1 => 0x0a,
        fconst_0 => 0x0b,
        fconst_1 => 0x0c,
        fconst_2 => 0x0d,
        dconst_0 => 0x0e,
        dconst_1 => 0x0f,
        bipush { val } => {
            args.push(val);
            0x10
        }
        sipush { val } => {
            args.push((val >> 8) as u8);
            args.push(val as u8);
            0x11
        }
        ldc { index } => {
            args.push(index);
            0x12
        }
        ldc_w { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0x13
        }
        ldc2_w { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0x14
        }
        iload { index } => {
            args.push(index);
            0x15
        }
        lload { index } => {
            args.push(index);
            0x16
        }
        fload { index } => {
            args.push(index);
            0x17
        }
        dload { index } => {
            args.push(index);
            0x18
        }
        aload { index } => {
            args.push(index);
            0x19
        }
        iload_0 => 0x1a,
        iload_1 => 0x1b,
        iload_2 => 0x1c,
        iload_3 => 0x1d,
        lload_0 => 0x1e,
        lload_1 => 0x1f,
        lload_2 => 0x20,
        lload_3 => 0x21,
        fload_0 => 0x22,
        fload_1 => 0x23,
        fload_2 => 0x24,
        fload_3 => 0x25,
        dload_0 => 0x26,
        dload_1 => 0x27,
        dload_2 => 0x28,
        dload_3 => 0x29,
        aload_0 => 0x2a,
        aload_1 => 0x2b,
        aload_2 => 0x2c,
        aload_3 => 0x2d,
        iaload => 0x2e,
        laload => 0x2f,
        faload => 0x30,
        daload => 0x31,
        aaload => 0x32,
        baload => 0x33,
        caload => 0x34,
        saload => 0x35,
        istore { index } => {
            args.push(index);
            0x36
        }
        lstore { index } => {
            args.push(index);
            0x37
        }
        fstore { index } => {
            args.push(index);
            0x38
        }
        dstore { index } => {
            args.push(index);
            0x39
        }
        astore { index } => {
            args.push(index);
            0x3a
        }
        istore_0 => 0x3b,
        istore_1 => 0x3c,
        istore_2 => 0x3d,
        istore_3 => 0x3e,
        lstore_0 => 0x3f,
        lstore_1 => 0x40,
        lstore_2 => 0x41,
        lstore_3 => 0x42,
        fstore_0 => 0x43,
        fstore_1 => 0x44,
        fstore_2 => 0x45,
        fstore_3 => 0x46,
        dstore_0 => 0x47,
        dstore_1 => 0x48,
        dstore_2 => 0x49,
        dstore_3 => 0x4a,
        astore_0 => 0x4b,
        astore_1 => 0x4c,
        astore_2 => 0x4d,
        astore_3 => 0x4e,
        iastore => 0x4f,
        lastore => 0x50,
        fastore => 0x51,
        dastore => 0x52,
        aastore => 0x53,
        bastore => 0x54,
        castore => 0x55,
        sastore => 0x56,
        pop => 0x57,
        pop2 => 0x58,
        dup => 0x59,
        dup_x1 => 0x5a,
        dup_x2 => 0x5b,
        dup2 => 0x5c,
        dup2_x1 => 0x5d,
        dup2_x2 => 0x5e,
        swap => 0x5f,
        iadd => 0x60,
        ladd => 0x61,
        fadd => 0x62,
        dadd => 0x63,
        isub => 0x64,
        lsub => 0x65,
        fsub => 0x66,
        dsub => 0x67,
        imul => 0x68,
        lmul => 0x69,
        fmul => 0x6a,
        dmul => 0x6b,
        idiv => 0x6c,
        ldiv => 0x6d,
        fdiv => 0x6e,
        ddiv => 0x6f,
        irem => 0x70,
        lrem => 0x71,
        frem => 0x72,
        drem => 0x73,
        ineg => 0x74,
        lneg => 0x75,
        fneg => 0x76,
        dneg => 0x77,
        ishl => 0x78,
        lshl => 0x79,
        ishr => 0x7a,
        lshr => 0x7b,
        iushr => 0x7c,
        lushr => 0x7d,
        iand => 0x7e,
        land => 0x7f,
        ior => 0x80,
        lor => 0x81,
        ixor => 0x82,
        lxor => 0x83,
        iinc { index, const_ } => {
            args.push(index);
            args.push(const_ as u8);
            0x84
        }
        i2l => 0x85,
        i2f => 0x86,
        i2d => 0x87,
        l2i => 0x88,
        l2f => 0x89,
        l2d => 0x8a,
        f2i => 0x8b,
        f2l => 0x8c,
        f2d => 0x8d,
        d2i => 0x8e,
        d2l => 0x8f,
        d2f => 0x90,
        i2b => 0x91,
        i2c => 0x92,
        i2s => 0x93,
        lcmp => 0x94,
        fcmpl => 0x95,
        fcmpg => 0x96,
        dcmpl => 0x97,
        dcmpg => 0x98,
        ifeq { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0x99
        }
        ifne { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0x9a
        }
        iflt { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0x9b
        }
        ifge { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0x9c
        }
        ifgt { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0x9d
        }
        ifle { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0x9e
        }
        if_icmpeq { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0x9f
        }
        if_icmpne { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xa0
        }
        if_icmplt { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xa1
        }
        if_icmpge { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xa2
        }
        if_icmpgt { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xa3
        }
        if_icmple { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xa4
        }
        if_acmpeq { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xa5
        }
        if_acmpne { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xa6
        }
        goto { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xa7
        }
        jsr { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xa8
        }
        ret { index } => {
            args.push(index);
            0xa9
        }
        tableswitch { default, low, high, jump_offsets, padding } => {
            #[allow(clippy::same_item_push)]
            for _ in 0..padding {
                args.push(0);
            }
            args.push((default >> 24) as u8);
            args.push((default >> 16) as u8);
            args.push((default >> 8) as u8);
            args.push(default as u8);
            args.push((low >> 24) as u8);
            args.push((low >> 16) as u8);
            args.push((low >> 8) as u8);
            args.push(low as u8);
            args.push((high >> 24) as u8);
            args.push((high >> 16) as u8);
            args.push((high >> 8) as u8);
            args.push(high as u8);
            for a in jump_offsets {
                args.push((a >> 24) as u8);
                args.push((a >> 16) as u8);
                args.push((a >> 8) as u8);
                args.push(a as u8);
            }
            0xaa
        }
        lookupswitch { default, match_offset_pairs, padding } => {
            #[allow(clippy::same_item_push)]
            for _ in 0..padding {
                args.push(0);
            }
            args.push((default >> 24) as u8);
            args.push((default >> 16) as u8);
            args.push((default >> 8) as u8);
            args.push(default as u8);
            for (a, b) in match_offset_pairs {
                args.push((a >> 24) as u8);
                args.push((a >> 16) as u8);
                args.push((a >> 8) as u8);
                args.push(a as u8);
                args.push((b >> 24) as u8);
                args.push((b >> 16) as u8);
                args.push((b >> 8) as u8);
                args.push(b as u8);
            }
            0xab
        }
        ireturn => 0xac,
        lreturn => 0xad,
        freturn => 0xae,
        dreturn => 0xaf,
        areturn => 0xb0,
        return_ => 0xb1,
        getstatic { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xb2
        }
        putstatic { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xb3
        }
        getfield { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xb4
        }
        putfield { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xb5
        }
        invokevirtual { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xb6
        }
        invokespecial { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xb7
        }
        invokestatic { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xb8
        }
        invokeinterface { index, count } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            args.push(count);
            args.push(0);
            0xb9
        }
        invokedynamic { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            args.push(0);
            args.push(0);
            0xba
        }
        new { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xbb
        }
        newarray { atype } => {
            args.push(atype);
            0xbc
        }
        anewarray { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xbd
        }
        arraylength => 0xbe,
        athrow => 0xbf,
        checkcast { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xc0
        }
        instanceof { index } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xc1
        }
        monitorenter => 0xc2,
        monitorexit => 0xc3,
        wide { opcode, index } => {
            args.push(opcode);
            args.push((index >> 8) as u8);
            args.push(index as u8);
            0xc4
        }
        wide_iinc { index, const_ } => {
            args.push(0x84);
            args.push((index >> 8) as u8);
            args.push(index as u8);
            args.push((const_ >> 8) as u8);
            args.push(const_ as u8);
            0xc4
        }
        multianewarray { index, dimensions } => {
            args.push((index >> 8) as u8);
            args.push(index as u8);
            args.push(dimensions);
            0xc5
        }
        ifnull { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xc6
        }
        ifnonnull { branch } => {
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xc7
        }
        goto_w { branch } => {
            args.push((branch >> 24) as u8);
            args.push((branch >> 16) as u8);
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xc8
        }
        jsr_w { branch } => {
            args.push((branch >> 24) as u8);
            args.push((branch >> 16) as u8);
            args.push((branch >> 8) as u8);
            args.push(branch as u8);
            0xc9
        }
        breakpoint => 0xca,
        reserved => 0xcb,
        impdep1 => 0xfe,
        impdep2 => 0xff
    };
    (byte, args)
}


