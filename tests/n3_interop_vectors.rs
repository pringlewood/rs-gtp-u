//! Byte-exact N3 vectors from the ex_packet_gateway lab and from TS 29.281 /
//! TS 38.415 so the codec is pinned against what real peers put on the wire.

use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr};

use rs_gtp_u::extension::{ExtensionHeader, PduSessionContainer};
use rs_gtp_u::{GtpuMessage, InformationElement, MessageType};

const TEID: u32 = 0x1122_3344;
const IPV4_PAYLOAD: [u8; 20] = [
    0x45, 0x00, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x40, 0x11, 0x00, 0x00, 0x0a, 0x2d, 0x00, 0x02,
    0x0a, 0x64, 0x00, 0x28,
];

fn decode(bytes: &[u8]) -> GtpuMessage {
    GtpuMessage::decode(&mut Cursor::new(bytes)).expect("decode")
}

fn encode(mut msg: GtpuMessage) -> Vec<u8> {
    let mut out = Vec::new();
    msg.encode(&mut out).expect("encode");
    out
}

fn with_payload(header: &[u8]) -> Vec<u8> {
    [header, &IPV4_PAYLOAD].concat()
}

/// ue-sim gtp_client.py GTPUPacket.encode() with sequence=0, and the
/// with-charging injector: flags 0x30, no optional fields.
#[test]
fn gpdu_flags_0x30_no_seq_no_ext() {
    let wire = with_payload(&[0x30, 0xFF, 0x00, 0x14, 0x11, 0x22, 0x33, 0x44]);

    let msg = decode(&wire);
    assert_eq!(msg.header.message_type, MessageType::Gpdu);
    assert_eq!(msg.header.teid, TEID);
    assert_eq!(msg.header.sequence_number, None);
    assert_eq!(msg.get_qfi(), None);
    assert_eq!(msg.payload, IPV4_PAYLOAD);

    assert_eq!(
        encode(GtpuMessage::create_gpdu(TEID, IPV4_PAYLOAD.to_vec())),
        wire
    );
}

/// ue-sim GTPUPacket.encode() with a non-zero sequence: flags 0x32.
#[test]
fn gpdu_flags_0x32_with_seq() {
    let wire = with_payload(&[
        0x32, 0xFF, 0x00, 0x18, 0x11, 0x22, 0x33, 0x44, 0x00, 0x07, 0x00, 0x00,
    ]);
    let msg = decode(&wire);
    assert_eq!(msg.header.sequence_number, Some(7));
    assert_eq!(msg.payload, IPV4_PAYLOAD);
}

/// gNB uplink G-PDU with a UL PDU SESSION INFORMATION container, QFI 9
/// (TS 38.415 5.5.2.2: octet 1 = PDU type 1 << 4, octet 2 = QFI).
#[test]
fn gpdu_uplink_psc_qfi9() {
    let wire = with_payload(&[
        0x34, 0xFF, 0x00, 0x1C, 0x11, 0x22, 0x33, 0x44, 0x00, 0x00, 0x00, 0x85, 0x01, 0x10, 0x09,
        0x00,
    ]);

    let msg = decode(&wire);
    assert_eq!(msg.get_qfi(), Some(9));
    let psc = msg.get_pdu_session_container().expect("psc");
    assert_eq!(psc.pdu_type, 1);
    assert_eq!(msg.payload, IPV4_PAYLOAD);

    let mut ul = GtpuMessage::create_gpdu(TEID, IPV4_PAYLOAD.to_vec());
    ul.add_extension_header(ExtensionHeader::PduSessionContainer(
        PduSessionContainer::new(1, 9, 0, false),
    ));
    assert_eq!(encode(ul), wire);
}

/// UPF downlink G-PDU with a DL PDU SESSION INFORMATION container, QFI 9
/// (TS 38.415 5.5.2.1: octet 1 = PDU type 0 << 4, octet 2 = PPP|RQI|QFI).
#[test]
fn gpdu_downlink_psc_qfi9() {
    let wire = with_payload(&[
        0x34, 0xFF, 0x00, 0x1C, 0x11, 0x22, 0x33, 0x44, 0x00, 0x00, 0x00, 0x85, 0x01, 0x00, 0x09,
        0x00,
    ]);

    let mut dl = GtpuMessage::create_gpdu(TEID, IPV4_PAYLOAD.to_vec());
    dl.add_extension_header(ExtensionHeader::PduSessionContainer(
        PduSessionContainer::new(0, 9, 0, false),
    ));
    assert_eq!(encode(dl), wire);
    assert_eq!(decode(&wire).get_qfi(), Some(9));
}

/// DL PSC with RQI and PPP/PPI set grows to two 4-byte units.
#[test]
fn gpdu_downlink_psc_rqi_ppi() {
    let wire = with_payload(&[
        0x34, 0xFF, 0x00, 0x20, 0x11, 0x22, 0x33, 0x44, 0x00, 0x00, 0x00, 0x85, 0x02, 0x00, 0xC9,
        0xA0, 0x00, 0x00, 0x00, 0x00,
    ]);

    let mut dl = GtpuMessage::create_gpdu(TEID, IPV4_PAYLOAD.to_vec());
    dl.add_extension_header(ExtensionHeader::PduSessionContainer(
        PduSessionContainer::new(0, 9, 5, true),
    ));
    assert_eq!(encode(dl), wire);

    let msg = decode(&wire);
    let psc = msg.get_pdu_session_container().expect("psc");
    assert_eq!((psc.qfi, psc.ppi, psc.rqi), (9, 5, true));
}

/// gNB UL PSC with QMP set carries timestamps (TS 38.415 5.5.2.2), so the
/// container is longer than one unit. The decoder must honour the length
/// byte and still find QFI and the payload.
#[test]
fn gpdu_uplink_psc_with_qmp_timestamps() {
    let mut header = vec![
        0x34, 0xFF, 0x00, 0x00, 0x11, 0x22, 0x33, 0x44, 0x00, 0x00, 0x00, 0x85, 0x05, 0x18, 0x09,
    ];
    header.extend_from_slice(&[0xAA; 16]);
    header.push(0x00);
    let len = (header.len() - 8 + IPV4_PAYLOAD.len()) as u16;
    header[2..4].copy_from_slice(&len.to_be_bytes());
    let wire = with_payload(&header);

    let msg = decode(&wire);
    assert_eq!(msg.get_qfi(), Some(9));
    assert_eq!(msg.payload, IPV4_PAYLOAD);
}

/// ex_packet_gateway pfcp/node.ex:571 encodes `{:gtp, :v1, :g_pdu, tei, 0, 0, [], data}`
/// with the gtp_packet codec, which sets S and PN because seq/n_pdu are not nil.
/// Bytes captured from GtpPacket.encode/1.
#[test]
fn gpdu_cp_function_seq0_npdu0() {
    let wire = [
        0x33, 0xFF, 0x00, 0x08, 0x11, 0x22, 0x33, 0x44, 0x00, 0x00, 0x00, 0x00, 0x45, 0x00, 0x00,
        0x14,
    ];
    let msg = decode(&wire);
    assert_eq!(msg.header.message_type, MessageType::Gpdu);
    assert_eq!(msg.header.teid, TEID);
    assert_eq!(msg.header.sequence_number, Some(0));
    assert_eq!(msg.header.n_pdu_number, Some(0));
    assert_eq!(msg.payload, [0x45, 0x00, 0x00, 0x14]);
}

/// TS 29.281 7.2.1/7.2.2: Echo Request carries S=1 and TEID 0; the Echo
/// Response echoes the sequence and carries Recovery (TV, restart counter 0).
#[test]
fn echo_request_response() {
    let req = [
        0x32, 0x01, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, 0x12, 0x34, 0x00, 0x00,
    ];
    let msg = decode(&req);
    assert_eq!(msg.header.message_type, MessageType::EchoRequest);
    assert_eq!(msg.header.sequence_number, Some(0x1234));
    assert_eq!(encode(GtpuMessage::create_echo_request(0x1234)), req);

    let resp = [
        0x32, 0x02, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x12, 0x34, 0x00, 0x00, 0x0E, 0x00,
    ];
    assert_eq!(encode(GtpuMessage::create_echo_response(0x1234, 0)), resp);
    let decoded = decode(&resp);
    assert_eq!(decoded.header.message_type, MessageType::EchoResponse);
    assert_eq!(
        decoded.information_elements,
        vec![InformationElement::Recovery(0)]
    );
}

/// TS 29.281 7.3.1: Error Indication has TEID 0 in the header, S=1, and
/// carries TEID Data I (the TEID of the offending G-PDU) and GTP-U Peer
/// Address (the sender's own address).
#[test]
fn error_indication_sent() {
    let wire = [
        0x32, 0x1A, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x11, 0x22,
        0x33, 0x44, 0x85, 0x00, 0x04, 0x0a, 0x64, 0x00, 0x1e,
    ];
    let ei = GtpuMessage::create_error_indication(TEID, IpAddr::V4(Ipv4Addr::new(10, 100, 0, 30)));
    assert_eq!(encode(ei), wire);
}

/// Error Indication from a gNB that adds the UDP Port extension header
/// (TS 29.281 5.2.2.1) ahead of the IEs.
#[test]
fn error_indication_received_with_udp_port_ext() {
    let wire = [
        0x36, 0x1A, 0x00, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x01, 0x08, 0x68,
        0x00, 0x10, 0x11, 0x22, 0x33, 0x44, 0x85, 0x00, 0x04, 0x0a, 0x64, 0x00, 0x0a,
    ];
    let msg = decode(&wire);
    assert_eq!(msg.header.message_type, MessageType::ErrorIndication);
    assert_eq!(msg.error_indication_teid(), Some(TEID));
    assert_eq!(
        msg.error_indication_peer(),
        Some(IpAddr::V4(Ipv4Addr::new(10, 100, 0, 10)))
    );
}

/// End Marker as sent by gNBs/UPFs: no optional fields, or a DL PSC in 5G.
#[test]
fn end_marker_received() {
    let plain = [0x30, 0xFE, 0x00, 0x00, 0x11, 0x22, 0x33, 0x44];
    let msg = decode(&plain);
    assert_eq!(msg.header.message_type, MessageType::EndMarker);
    assert_eq!(msg.header.teid, TEID);

    let with_psc = [
        0x34, 0xFE, 0x00, 0x08, 0x11, 0x22, 0x33, 0x44, 0x00, 0x00, 0x00, 0x85, 0x01, 0x00, 0x09,
        0x00,
    ];
    let msg = decode(&with_psc);
    assert_eq!(msg.header.message_type, MessageType::EndMarker);
    assert_eq!(msg.get_qfi(), Some(9));
}

#[test]
fn end_marker_sent() {
    assert_eq!(
        encode(GtpuMessage::create_end_marker(TEID, 3)),
        [0x32, 0xFE, 0x00, 0x04, 0x11, 0x22, 0x33, 0x44, 0x00, 0x03, 0x00, 0x00]
    );
}
