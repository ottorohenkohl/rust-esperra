use embassy_net::Runner;
use embassy_time::{Duration, Timer};
use esp_radio::wifi::{Interface, WifiController};

use embassy_executor::task;

#[task]
pub async fn connection_task(mut controller: WifiController<'static>) {
    loop {
        if controller.connect_async().await.is_ok() {
            let _ = controller.wait_for_disconnect_async().await;
        }

        Timer::after(Duration::from_secs(5)).await;
    }
}

#[task]
pub async fn network_task(mut runner: Runner<'static, Interface<'static>>) {
    runner.run().await;
}
