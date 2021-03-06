extern crate laminar;

use laminar::{
    net::{NetworkConfig, UdpSocket},
    DeliveryMethod, Packet,
};
use std::{
    net::SocketAddr,
    sync::mpsc::Receiver,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// This is an test server we use to receive data from clients.
pub struct ServerMoq {
    config: NetworkConfig,
    host: SocketAddr,
    non_blocking: bool,
}

impl ServerMoq {
    pub fn new(config: NetworkConfig, non_blocking: bool, host: SocketAddr) -> Self {
        ServerMoq {
            config,
            host,
            non_blocking,
        }
    }

    pub fn start_receiving(
        &mut self,
        cancellation_channel: Receiver<bool>,
        expected_payload: Vec<u8>,
    ) -> JoinHandle<u32> {
        let mut udp_socket: UdpSocket = UdpSocket::bind(self.host, self.config.clone()).unwrap();
        udp_socket.set_nonblocking(self.non_blocking).unwrap();

        let mut packet_throughput = 0;
        let mut packets_total_received = 0;
        let mut second_counter = Instant::now();

        thread::spawn(move || {
            loop {
                let result = udp_socket.recv();

                match result {
                    Ok(Some(packet)) => {
                        assert_eq!(packet.payload(), expected_payload.as_slice());
                        packets_total_received += 1;
                        packet_throughput += 1;

                        udp_socket.send(&packet).unwrap();
                    }
                    Ok(None) => {}
                    Err(_e) => match cancellation_channel.try_recv() {
                        Ok(val) => {
                            if val == true {
                                return packets_total_received;
                            }
                        }
                        Err(_e) => {}
                    },
                }

                if second_counter.elapsed().as_secs() >= 1 {
                    // reset counter
                    second_counter = Instant::now();

                    packet_throughput = 0;
                }
            }
        })
    }

    pub fn add_client(&self, data: Vec<u8>, client_stub: ClientStub) -> JoinHandle<()> {
        let packets_to_send = client_stub.packets_to_send;
        let host = self.host;
        let data_to_send = data;
        let config = self.config.clone();
        thread::spawn(move || {
            let mut client = UdpSocket::bind(client_stub.endpoint, config.clone()).unwrap();
            let _result = client.set_nonblocking(true);

            let len = data_to_send.len();

            for _ in 0..packets_to_send {
                let result = client.recv();

                match result {
                    Ok(Some(packet)) => {
                        assert_eq!(packet.payload(), data_to_send.as_slice());
                        assert_eq!(packet.addr(), host);
                    }
                    Ok(None) => {}
                    Err(_) => {}
                }

                let send_result = client.send(&Packet::new(
                    host,
                    data_to_send.clone().into_boxed_slice(),
                    client_stub.packet_delivery,
                ));

                if len <= config.fragment_size as usize {
                    send_result.is_ok();
                } else {
                    // if fragment, todo: add size assert.
                    send_result.is_ok();
                }

                thread::sleep(client_stub.timeout_sending);
            }
        })
    }
}

pub struct ClientStub {
    timeout_sending: Duration,
    endpoint: SocketAddr,
    packets_to_send: u32,
    packet_delivery: DeliveryMethod,
}

impl ClientStub {
    pub fn new(
        timeout_sending: Duration,
        endpoint: SocketAddr,
        packets_to_send: u32,
        packet_delivery: DeliveryMethod,
    ) -> ClientStub {
        ClientStub {
            timeout_sending,
            endpoint,
            packets_to_send,
            packet_delivery,
        }
    }
}
