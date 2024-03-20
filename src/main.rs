//! This example uses the RP Pico W board Wifi chip (cyw43).
//! Connects to specified Wifi network and creates a TCP endpoint on port 1234.

#![no_std]
#![no_main]
#![allow(async_fn_in_trait)]

mod config;

use config::{WIFI_NETWORK, WIFI_PASSWORD};

use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::{Config, Ipv4Address, Stack, StackResources, DhcpConfig};
use rust_mqtt::client::client_config::MqttVersion::MQTTv5;
use rust_mqtt::utils::rng_generator::CountingRng;
use rust_mqtt::{
    client::{client::MqttClient, client_config::ClientConfig},
    packet::v5::publish_packet::QualityOfService,
};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output};
// use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_25, PIO0};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;
use heapless::String;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});


#[embassy_executor::task]
async fn wifi_task(
    runner: cyw43::Runner<'static, Output<'static, PIN_23>, PioSpi<'static, PIN_25, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}


#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("Hello World!");

    let p = embassy_rp::init(Default::default());

    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(&mut pio.common, pio.sm0, pio.irq0, cs, p.PIN_24, p.PIN_29, p.DMA_CH0);

    let mut relay = Output::new(p.PIN_6, Level::Low);

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(wifi_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let mut dhcp_config = DhcpConfig::default();
    let hostname: String<32> = String::try_from("PicoWSwitchRS").unwrap();
    dhcp_config.hostname = Some(hostname);
    let config = Config::dhcpv4(dhcp_config);

    // Generate random seed
    let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guarenteed to be random.

    // Init network stack
    static STACK: StaticCell<Stack<cyw43::NetDriver<'static>>> = StaticCell::new();
    static RESOURCES: StaticCell<StackResources<2>> = StaticCell::new();
    let stack = &*STACK.init(Stack::new(
        net_device,
        config,
        RESOURCES.init(StackResources::<2>::new()),
        seed,
    ));

    unwrap!(spawner.spawn(net_task(stack)));

    loop {
        //control.join_open(WIFI_NETWORK).await;
        match control.join_wpa2(WIFI_NETWORK, WIFI_PASSWORD).await {
            Ok(_) => break,
            Err(err) => {
                info!("join failed with status={}", err.status);
            }
        }
    }

    // Wait for DHCP, not necessary when using static IP
    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    info!("DHCP is now up!");


    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut buf = [0; 4096];

    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(Duration::from_secs(10)));

    let ip = "192.168.5.85";
    let port = "1883";

    defmt::info!("Creating sockets");
    let addr = (Ipv4Address::new(192,168,5,85), 1883);

    let socket = socket.connect(addr);
    let conn_pub = socket.await.unwrap();
    // let conn_recv = unsafe { socket.connect(addr).await };

    let mut config = ClientConfig::new(MQTTv5, CountingRng(0));
    // config.add_qos(QualityOfService::QoS0);
    config.add_max_subscribe_qos(QualityOfService::QoS0);
    config.add_username("tasmota_plug");
    config.add_password("plugs");
    config.keep_alive = u16::MAX;
    let mut recv_buffer = [0; 1000];
    let mut write_buffer = [0; 1000];

    let mut client = MqttClient::<_, 20, CountingRng>::new(
        socket,
        &mut write_buffer,
        1000,
        &mut recv_buffer,
        1000,
        config,
    );
    defmt::info!("[PUBLISHER] Connecting to broker");
    client.connect_to_broker().await.unwrap();

    defmt::info!("[PUBLISHER] sending message");
    client.send_message("test-topic", "{'temp':42}").await.unwrap();
    defmt::info!("[PUBLISHER] message sent");
}
