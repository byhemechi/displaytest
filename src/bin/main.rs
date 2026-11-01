#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use bt_hci::controller::ExternalController;
use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::Level;
use esp_hal::timer::timg::TimerGroup;
use esp_radio::ble::controller::BleConnector;
use panic_rtt_target as _;
use trouble_host::prelude::*;

extern crate alloc;

const CONNECTIONS_MAX: usize = 1;
const L2CAP_CHANNELS_MAX: usize = 1;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.3.0
    // generator parameters: --chip esp32s3 -o unstable-hal -o alloc -o embassy -o ble-trouble -o probe-rs -o defmt -o panic-rtt-target -o embedded-test -o zed

    rtt_target::rtt_init_defmt!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("Embassy initialized!");

    // find more examples https://github.com/embassy-rs/trouble/tree/main/examples/esp32
    let transport = BleConnector::new(peripherals.BT, Default::default()).unwrap();
    let ble_controller = ExternalController::<_, 1>::new(transport);
    let mut resources: HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX> =
        HostResources::new();
    let _stack = trouble_host::new(ble_controller, &mut resources);

    // TODO: Spawn some tasks
    let _ = spawner;

    let pin_scl = peripherals.GPIO4;
    let pin_sda = peripherals.GPIO5;
    let pin_res = peripherals.GPIO6;
    let pin_dc = peripherals.GPIO7;
    let pin_cs = peripherals.GPIO15;
    let pin_bl = peripherals.GPIO16;

    let output_config = esp_hal::gpio::OutputConfig::default();
    let rst = esp_hal::gpio::Output::new(pin_res, Level::Low, output_config);
    let dc = esp_hal::gpio::Output::new(pin_dc, Level::Low, output_config);
    let cs = esp_hal::gpio::Output::new(pin_cs, Level::Low, output_config);
    let mut _backlight = esp_hal::gpio::Output::new(pin_bl, Level::High, output_config);

    info!("about to init spi!");
    let spi_config =
        esp_hal::spi::master::Config::default().with_frequency(esp_hal::time::Rate::from_mhz(80));
    let spi = esp_hal::spi::master::Spi::new(peripherals.SPI2, spi_config)
        .unwrap()
        .with_sck(pin_scl)
        .with_mosi(pin_sda)
        .with_cs(cs);

    let mut delay = esp_hal::delay::Delay::new();
    let mut buffer = [0_u8; 172 * 320];

    info!("buffer size: {:?}", buffer.len());

    let spi_device = embedded_hal_bus::spi::ExclusiveDevice::new(spi, NoCs, delay).unwrap();
    let di = mipidsi::interface::SpiInterface::new(spi_device, dc, &mut buffer);

    let mut display = mipidsi::Builder::new(mipidsi::models::ST7789, di)
        .display_size(172 as u16, 320 as u16)
        .reset_pin(rst)
        // .orientation(
        //     mipidsi::options::Orientation::new().rotate(mipidsi::options::Rotation::Deg270),
        // )
        .invert_colors(mipidsi::options::ColorInversion::Inverted)
        .display_offset(34, 0)
        .init(&mut delay)
        .unwrap();

    info!("display: {:?}", &display.bounding_box());
    display.clear(Rgb565::BLACK).unwrap();

    let image_data = include_bytes!("x.bmp");

    let image = tinybmp::Bmp::from_slice(image_data).unwrap();

    let image = embedded_graphics::image::Image::new(&image, Point::zero());

    image.draw(&mut display).unwrap();

    loop {}

    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.1.0/examples
}

struct NoCs;

impl embedded_hal::digital::OutputPin for NoCs {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl embedded_hal::digital::ErrorType for NoCs {
    type Error = core::convert::Infallible;
}
