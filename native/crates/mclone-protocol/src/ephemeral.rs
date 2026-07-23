use mclone_core::Vec3d;

use crate::{ProtocolCodecError, ProtocolCodecResult, RemotePlayerId};

const CLIENT_BODY_POSE_TAG: u8 = 1;
const SERVER_REMOTE_BODY_POSE_TAG: u8 = 1;

pub const MAX_EPHEMERAL_MESSAGE_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EffectiveEphemeralTransport {
    #[default]
    ReliableFallback,
    InMemory,
    NativeUdp,
    WebTransport,
    WebRtcDataChannel,
}

impl EffectiveEphemeralTransport {
    pub const fn tag(self) -> u8 {
        match self {
            Self::ReliableFallback => 0,
            Self::InMemory => 1,
            Self::NativeUdp => 2,
            Self::WebTransport => 3,
            Self::WebRtcDataChannel => 4,
        }
    }

    pub const fn from_tag(tag: u8) -> ProtocolCodecResult<Self> {
        match tag {
            0 => Ok(Self::ReliableFallback),
            1 => Ok(Self::InMemory),
            2 => Ok(Self::NativeUdp),
            3 => Ok(Self::WebTransport),
            4 => Ok(Self::WebRtcDataChannel),
            _ => Err(ProtocolCodecError::InvalidData(
                "unknown effective ephemeral transport",
            )),
        }
    }

    pub const fn is_mixed_reliability(self) -> bool {
        matches!(
            self,
            Self::InMemory | Self::NativeUdp | Self::WebTransport | Self::WebRtcDataChannel
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerBodyPoseSample {
    pub presentation_epoch: u32,
    pub sequence: u32,
    pub sample_time_millis: u32,
    pub position: Vec3d,
    pub y_rot_degrees: f32,
    pub x_rot_degrees: f32,
    pub on_ground: bool,
    pub discontinuity: bool,
}

impl PlayerBodyPoseSample {
    pub const fn new(
        presentation_epoch: u32,
        sequence: u32,
        sample_time_millis: u32,
        position: Vec3d,
        y_rot_degrees: f32,
        x_rot_degrees: f32,
        on_ground: bool,
    ) -> Self {
        Self {
            presentation_epoch,
            sequence,
            sample_time_millis,
            position,
            y_rot_degrees,
            x_rot_degrees,
            on_ground,
            discontinuity: false,
        }
    }

    pub const fn with_discontinuity(mut self, discontinuity: bool) -> Self {
        self.discontinuity = discontinuity;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RemotePlayerBodyPoseSample {
    pub id: RemotePlayerId,
    pub pose: PlayerBodyPoseSample,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClientEphemeralMessage {
    BodyPose(PlayerBodyPoseSample),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ServerEphemeralMessage {
    RemoteBodyPose(RemotePlayerBodyPoseSample),
}

pub fn sequence_is_newer(candidate: u32, current: u32) -> bool {
    let distance = candidate.wrapping_sub(current);
    distance != 0 && distance < (1 << 31)
}

pub fn encode_client_ephemeral_message(
    message: ClientEphemeralMessage,
) -> ProtocolCodecResult<Vec<u8>> {
    let mut writer = EphemeralWriter::default();
    match message {
        ClientEphemeralMessage::BodyPose(sample) => {
            validate_body_pose_sample(sample)?;
            writer.u8(CLIENT_BODY_POSE_TAG);
            writer.body_pose(sample);
        }
    }
    Ok(writer.finish())
}

pub fn decode_client_ephemeral_message(
    bytes: &[u8],
) -> ProtocolCodecResult<ClientEphemeralMessage> {
    checked_message_size(bytes)?;
    let mut reader = EphemeralReader::new(bytes);
    let message = match reader.u8()? {
        CLIENT_BODY_POSE_TAG => ClientEphemeralMessage::BodyPose(reader.body_pose()?),
        _ => {
            return Err(ProtocolCodecError::InvalidData(
                "unknown client ephemeral message tag",
            ));
        }
    };
    reader.finish()?;
    Ok(message)
}

pub fn encode_server_ephemeral_message(
    message: ServerEphemeralMessage,
) -> ProtocolCodecResult<Vec<u8>> {
    let mut writer = EphemeralWriter::default();
    match message {
        ServerEphemeralMessage::RemoteBodyPose(sample) => {
            validate_body_pose_sample(sample.pose)?;
            writer.u8(SERVER_REMOTE_BODY_POSE_TAG);
            writer.u64(sample.id.0);
            writer.body_pose(sample.pose);
        }
    }
    Ok(writer.finish())
}

pub fn decode_server_ephemeral_message(
    bytes: &[u8],
) -> ProtocolCodecResult<ServerEphemeralMessage> {
    checked_message_size(bytes)?;
    let mut reader = EphemeralReader::new(bytes);
    let message = match reader.u8()? {
        SERVER_REMOTE_BODY_POSE_TAG => {
            ServerEphemeralMessage::RemoteBodyPose(RemotePlayerBodyPoseSample {
                id: RemotePlayerId(reader.u64()?),
                pose: reader.body_pose()?,
            })
        }
        _ => {
            return Err(ProtocolCodecError::InvalidData(
                "unknown server ephemeral message tag",
            ));
        }
    };
    reader.finish()?;
    Ok(message)
}

pub fn validate_body_pose_sample(sample: PlayerBodyPoseSample) -> ProtocolCodecResult<()> {
    if sample.presentation_epoch == 0 {
        return Err(ProtocolCodecError::InvalidData(
            "body pose presentation epoch must be nonzero",
        ));
    }
    if sample.sequence == 0 {
        return Err(ProtocolCodecError::InvalidData(
            "body pose sequence must be nonzero",
        ));
    }
    if !sample.position.is_finite()
        || !sample.y_rot_degrees.is_finite()
        || !sample.x_rot_degrees.is_finite()
    {
        return Err(ProtocolCodecError::InvalidData(
            "body pose contains non-finite value",
        ));
    }
    Ok(())
}

fn checked_message_size(bytes: &[u8]) -> ProtocolCodecResult<()> {
    if bytes.len() > MAX_EPHEMERAL_MESSAGE_BYTES {
        return Err(ProtocolCodecError::InvalidData(
            "ephemeral message exceeds the protocol maximum",
        ));
    }
    Ok(())
}

#[derive(Default)]
struct EphemeralWriter {
    bytes: Vec<u8>,
}

impl EphemeralWriter {
    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn body_pose(&mut self, sample: PlayerBodyPoseSample) {
        self.u32(sample.presentation_epoch);
        self.u32(sample.sequence);
        self.u32(sample.sample_time_millis);
        self.f64(sample.position.x);
        self.f64(sample.position.y);
        self.f64(sample.position.z);
        self.f32(sample.y_rot_degrees);
        self.f32(sample.x_rot_degrees);
        self.bool(sample.on_ground);
        self.bool(sample.discontinuity);
    }
}

struct EphemeralReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> EphemeralReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn finish(self) -> ProtocolCodecResult<()> {
        let remaining = self.bytes.len().saturating_sub(self.offset);
        if remaining == 0 {
            Ok(())
        } else {
            Err(ProtocolCodecError::TrailingBytes { remaining })
        }
    }

    fn exact<const N: usize>(&mut self) -> ProtocolCodecResult<[u8; N]> {
        let remaining = self.bytes.len().saturating_sub(self.offset);
        if remaining < N {
            return Err(ProtocolCodecError::UnexpectedEof {
                needed: N,
                remaining,
            });
        }
        let mut out = [0; N];
        out.copy_from_slice(&self.bytes[self.offset..self.offset + N]);
        self.offset += N;
        Ok(out)
    }

    fn u8(&mut self) -> ProtocolCodecResult<u8> {
        Ok(self.exact::<1>()?[0])
    }

    fn bool(&mut self) -> ProtocolCodecResult<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ProtocolCodecError::InvalidData(
                "boolean value must be 0 or 1",
            )),
        }
    }

    fn u32(&mut self) -> ProtocolCodecResult<u32> {
        Ok(u32::from_le_bytes(self.exact::<4>()?))
    }

    fn u64(&mut self) -> ProtocolCodecResult<u64> {
        Ok(u64::from_le_bytes(self.exact::<8>()?))
    }

    fn f32(&mut self) -> ProtocolCodecResult<f32> {
        Ok(f32::from_le_bytes(self.exact::<4>()?))
    }

    fn f64(&mut self) -> ProtocolCodecResult<f64> {
        Ok(f64::from_le_bytes(self.exact::<8>()?))
    }

    fn body_pose(&mut self) -> ProtocolCodecResult<PlayerBodyPoseSample> {
        let sample = PlayerBodyPoseSample {
            presentation_epoch: self.u32()?,
            sequence: self.u32()?,
            sample_time_millis: self.u32()?,
            position: Vec3d::new(self.f64()?, self.f64()?, self.f64()?),
            y_rot_degrees: self.f32()?,
            x_rot_degrees: self.f32()?,
            on_ground: self.bool()?,
            discontinuity: self.bool()?,
        };
        validate_body_pose_sample(sample)?;
        Ok(sample)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(sequence: u32) -> PlayerBodyPoseSample {
        PlayerBodyPoseSample::new(
            7,
            sequence,
            1_234,
            Vec3d::new(1.25, 64.0, -3.5),
            45.0,
            -10.0,
            true,
        )
    }

    #[test]
    fn client_and_server_messages_round_trip() {
        let client = ClientEphemeralMessage::BodyPose(sample(9));
        assert_eq!(
            decode_client_ephemeral_message(&encode_client_ephemeral_message(client).unwrap())
                .unwrap(),
            client
        );

        let server = ServerEphemeralMessage::RemoteBodyPose(RemotePlayerBodyPoseSample {
            id: RemotePlayerId(42),
            pose: sample(10).with_discontinuity(true),
        });
        assert_eq!(
            decode_server_ephemeral_message(&encode_server_ephemeral_message(server).unwrap())
                .unwrap(),
            server
        );
    }

    #[test]
    fn malformed_body_samples_are_rejected() {
        assert!(validate_body_pose_sample(sample(0)).is_err());
        assert!(
            validate_body_pose_sample(PlayerBodyPoseSample {
                position: Vec3d::new(f64::NAN, 0.0, 0.0),
                ..sample(1)
            })
            .is_err()
        );

        let mut trailing =
            encode_client_ephemeral_message(ClientEphemeralMessage::BodyPose(sample(1))).unwrap();
        trailing.push(0);
        assert!(decode_client_ephemeral_message(&trailing).is_err());
    }

    #[test]
    fn sequence_comparison_handles_wrap_without_accepting_duplicates() {
        assert!(sequence_is_newer(2, 1));
        assert!(!sequence_is_newer(1, 1));
        assert!(sequence_is_newer(1, u32::MAX));
        assert!(!sequence_is_newer(u32::MAX, 1));
    }

    #[test]
    fn effective_transport_tags_are_strict() {
        for transport in [
            EffectiveEphemeralTransport::ReliableFallback,
            EffectiveEphemeralTransport::InMemory,
            EffectiveEphemeralTransport::NativeUdp,
            EffectiveEphemeralTransport::WebTransport,
            EffectiveEphemeralTransport::WebRtcDataChannel,
        ] {
            assert_eq!(
                EffectiveEphemeralTransport::from_tag(transport.tag()).unwrap(),
                transport
            );
        }
        assert!(EffectiveEphemeralTransport::from_tag(5).is_err());
    }
}
