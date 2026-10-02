use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use uuid::Uuid;

use super::protocol::{GatewayFrame, PROTOCOL_VERSION};

pub const MAX_FRAME_SIZE: usize = 64 * 1024; // 64 KiB
const HEADER_LEN: usize = 4 + 1 + 1 + 16 + 8; // 30 bytes

pub struct GatewayCodec;

impl Encoder<GatewayFrame> for GatewayCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: GatewayFrame, dst: &mut BytesMut) -> Result<(), Self::Error> {
        if item.payload.len() > MAX_FRAME_SIZE - HEADER_LEN {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Payload too large",
            ));
        }

        dst.reserve(HEADER_LEN + item.payload.len());
        dst.put_u32(item.payload.len() as u32);
        dst.put_u8(item.protocol_version);
        dst.put_u8(item.msg_type);
        dst.put_slice(item.request_id.as_bytes());
        dst.put_u64(item.sequence);
        dst.put_slice(&item.payload);

        Ok(())
    }
}

impl Decoder for GatewayCodec {
    type Item = GatewayFrame;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < HEADER_LEN {
            return Ok(None);
        }

        let payload_len = u32::from_be_bytes(src[0..4].try_into().unwrap()) as usize;

        if payload_len > MAX_FRAME_SIZE - HEADER_LEN {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Frame too large",
            ));
        }

        if src.len() < HEADER_LEN + payload_len {
            src.reserve(HEADER_LEN + payload_len - src.len());
            return Ok(None);
        }

        // We have a full frame. Consume it.
        src.advance(4); // payload_len
        let protocol_version = src.get_u8();
        let msg_type = src.get_u8();

        let mut uuid_bytes = [0u8; 16];
        src.copy_to_slice(&mut uuid_bytes);
        let request_id = Uuid::from_bytes(uuid_bytes);

        let sequence = src.get_u64();

        let mut payload = vec![0u8; payload_len];
        if payload_len > 0 {
            src.copy_to_slice(&mut payload);
        }

        if protocol_version != PROTOCOL_VERSION {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid protocol version",
            ));
        }

        Ok(Some(GatewayFrame {
            protocol_version,
            msg_type,
            request_id,
            sequence,
            payload,
        }))
    }
}
