//! Integrační testy racc-core: QUIC loopback, handshake + šifrovaná obálka.

use racc_core::crypto::{handshake_finish, handshake_initiate, handshake_respond, SessionCipher};
use racc_core::net::{
    client_endpoint, generate_self_signed, read_frame, server_endpoint, write_frame,
};
use racc_core::proto::{Envelope, Kind, TtyData};

#[tokio::test]
async fn quic_loopback_frame_roundtrip() {
    let (cert, key) = generate_self_signed("localhost").unwrap();
    let server = server_endpoint("127.0.0.1:0".parse().unwrap(), &cert, &key).unwrap();
    let server_addr = server.local_addr().unwrap();

    // Endpoint i serverové spojení musí zůstat naživu, dokud klient nedočte
    // (drop poslední Connection = CONNECTION_CLOSE).
    let srv_endpoint = server.clone();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
    let srv = tokio::spawn(async move {
        let incoming = srv_endpoint.accept().await.expect("accept");
        let conn = incoming.await.expect("connect");
        let (mut send, mut recv) = conn.accept_bi().await.expect("accept_bi");
        let frame = read_frame(&mut recv).await.expect("read");
        write_frame(&mut send, &frame).await.expect("write");
        send.finish().ok();
        let _ = done_rx.await;
        conn.close(0u32.into(), b"done");
    });

    let client = client_endpoint("127.0.0.1:0".parse().unwrap(), &cert).unwrap();
    let conn = client
        .connect(server_addr, "localhost")
        .unwrap()
        .await
        .unwrap();
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    write_frame(&mut send, b"ping-pong").await.unwrap();
    send.finish().unwrap();
    let back = read_frame(&mut recv).await.unwrap();
    assert_eq!(back, b"ping-pong");
    let _ = done_tx.send(());
    srv.await.unwrap();
    client.wait_idle().await;
}

#[tokio::test]
async fn handshake_then_encrypted_envelope() {
    use racc_core::crypto::Identity;

    let a = Identity::generate();
    let b = Identity::generate();
    let (eph_a, init) = handshake_initiate(&a);
    let (resp, key_b, _) = handshake_respond(&b, &init).unwrap();
    let (key_a, _) = handshake_finish(&eph_a, &init, &resp).unwrap();
    assert_eq!(key_a, key_b);

    // A zašifruje obálku, B dešifruje.
    let ca = SessionCipher::new(&key_a);
    let cb = SessionCipher::new(&key_b);
    let env = Envelope::wrap(
        Kind::Tty,
        b"0123456789abcdef",
        &TtyData {
            pty_id: 1,
            data: b"whoami\n".to_vec(),
            resize: None,
            eof: false,
        },
    );
    let packet = ca.encrypt(&env.encode_vec());
    let plain = cb.decrypt(&packet).unwrap();
    let decoded = Envelope::decode_vec(&plain).unwrap();
    let tty: TtyData = decoded.unwrap(Kind::Tty).unwrap();
    assert_eq!(tty.data, b"whoami\n");
}
