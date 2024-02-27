// See the "macOS permissions note" in README.md before running this on macOS
// Big Sur or later.

use btleplug::api::{bleuuid::BleUuid, Central, CentralEvent, Manager as _, ScanFilter, Peripheral};
use btleplug::platform::{Adapter, Manager};
use futures::stream::StreamExt;
use std::error::Error;

async fn get_central(manager: &Manager) -> Adapter {
    let adapters = manager.adapters().await.unwrap();
    adapters.into_iter().nth(0).unwrap()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let manager = Manager::new().await?;

    // get the first bluetooth adapter
    // connect to the adapter
    let central = get_central(&manager).await;

    let central_state = central.adapter_state().await.unwrap();
    println!("CentralState: {:?}", central_state);

    // Each adapter has an event stream, we fetch via events(),
    // simplifying the type, this will return what is essentially a
    // Future<Result<Stream<Item=CentralEvent>>>.
    let mut events = central.events().await?;

    // start scanning for devices
    // central.start_scan(ScanFilter::default()).await?;

    // Print based on whatever the event receiver outputs. Note that the event
    // receiver blocks, so in a real program, this should be run in its own
    // thread (not task, as this library does not yet use async channels).
    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                println!("DeviceDiscovered: {:?}, {}", id, id.to_string());
                // if id.to_string() == "f68df577-b2d7-8705-216e-fa4022461a99".to_string() {
                if id.to_string() == "hci0/dev_F2_CF_49_B0_5D_AF".to_string() {
                    println!("trezor found");
                    let dev = central.peripheral(&id).await.unwrap();
                    if let Err(e) = dev.connect_with_timeout(std::time::Duration::from_secs(5)).await {
                        println!("conn {:?}", e);
                        let d = dev.disconnect().await;
                        println!("disconn {:?}", d);
                    }
                }
            }
            CentralEvent::StateUpdate(state) => {
                println!("AdapterStatusUpdate {:?}", state);
                let state2 = central.adapter_state().await.unwrap();
                println!("AdapterStatusUpdateGetter {:?}", state2);
            }
            CentralEvent::StateUpdate(state) => {
                println!("AdapterStatusUpdate {:?}", state);
            }
            CentralEvent::DeviceConnected(id) => {
                println!("DeviceConnected: {:?}", id);
            }
            CentralEvent::DeviceDisconnected(id) => {
                println!("DeviceDisconnected: {:?}", id);
            }
            // CentralEvent::ManufacturerDataAdvertisement {
            //     id,
            //     manufacturer_data,
            // } => {
            //     println!(
            //         "ManufacturerDataAdvertisement: {:?}, {:?}",
            //         id, manufacturer_data
            //     );
            // }
            // CentralEvent::ServiceDataAdvertisement { id, service_data } => {
            //     println!("ServiceDataAdvertisement: {:?}, {:?}", id, service_data);
            // }
            // CentralEvent::ServicesAdvertisement { id, services } => {
            //     let services: Vec<String> =
            //         services.into_iter().map(|s| s.to_short_string()).collect();
            //     println!("ServicesAdvertisement: {:?}, {:?}", id, services);
            // }
            _ => {}
        }
    }
    Ok(())
}
