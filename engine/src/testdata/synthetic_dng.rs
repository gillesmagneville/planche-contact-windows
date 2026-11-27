// Body of raw::tests::synthetic_dng (included): an uncompressed 64 x 48 DNG.
{
    const W: u32 = 64;
    const H: u32 = 48;
    const RAW_VALUE: u16 = 20000;
        const SHORT: u16 = 3;
        const LONG: u16 = 4;
        const ASCII: u16 = 2;
        const BYTE: u16 = 1;
        const RATIONAL: u16 = 5;
        const SRATIONAL: u16 = 10;

        let mut buf: Vec<u8> = b"II*\0\0\0\0\0".to_vec();
        for _ in 0..(W * H) {
            buf.extend_from_slice(&RAW_VALUE.to_le_bytes());
        }
        let put = |buf: &mut Vec<u8>, bytes: &[u8]| -> u32 {
            if buf.len() % 2 == 1 {
                buf.push(0);
            }
            let off = buf.len() as u32;
            buf.extend_from_slice(bytes);
            off
        };
        let make = put(&mut buf, b"FakeCam\0");
        let model = put(&mut buf, b"Synthetic DNG\0");
        let mut cm = Vec::new();
        for (n, d) in [(1i32, 1i32), (0, 1), (0, 1), (0, 1), (1, 1), (0, 1), (0, 1), (0, 1), (1, 1)] {
            cm.extend_from_slice(&n.to_le_bytes());
            cm.extend_from_slice(&d.to_le_bytes());
        }
        let cm_off = put(&mut buf, &cm);
        let mut asn = Vec::new();
        for _ in 0..3 {
            asn.extend_from_slice(&1u32.to_le_bytes());
            asn.extend_from_slice(&1u32.to_le_bytes());
        }
        let asn_off = put(&mut buf, &asn);

        let mut ent: Vec<(u16, u16, u32, u32)> = vec![
            (254, LONG, 1, 0),
            (256, LONG, 1, W),
            (257, LONG, 1, H),
            (258, SHORT, 1, 16),
            (259, SHORT, 1, 1),
            (262, SHORT, 1, 32803),
            (271, ASCII, 8, make),
            (272, ASCII, 14, model),
            (273, LONG, 1, 8),
            (274, SHORT, 1, 1),
            (277, SHORT, 1, 1),
            (278, LONG, 1, H),
            (279, LONG, 1, W * H * 2),
            (284, SHORT, 1, 1),
            (33421, SHORT, 2, 2 | (2 << 16)),
            (33422, BYTE, 4, 0 | (1 << 8) | (1 << 16) | (2 << 24)),
            (50706, BYTE, 4, 1 | (4 << 8)),
            (50708, ASCII, 14, model),
            (50714, SHORT, 1, 0),
            (50717, LONG, 1, 65535),
            (50721, SRATIONAL, 9, cm_off),
            (50728, RATIONAL, 3, asn_off),
            (50778, SHORT, 1, 21),
        ];
        ent.sort();
        if buf.len() % 2 == 1 {
            buf.push(0);
        }
        let ifd_off = buf.len() as u32;
        buf.extend_from_slice(&(ent.len() as u16).to_le_bytes());
        for (t, ty, c, v) in ent {
            buf.extend_from_slice(&t.to_le_bytes());
            buf.extend_from_slice(&ty.to_le_bytes());
            buf.extend_from_slice(&c.to_le_bytes());
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf[4..8].copy_from_slice(&ifd_off.to_le_bytes());
        buf
}
