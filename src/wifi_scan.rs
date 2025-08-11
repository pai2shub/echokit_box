use esp_idf_svc::wifi::ClientConfiguration;
use esp_idf_svc::wifi::Configuration;
use esp_idf_svc::wifi::WifiDriver;

pub fn scan(
    modem: esp_idf_svc::hal::modem::Modem,
    sysloop: esp_idf_svc::eventloop::EspSystemEventLoop,
) {
    log::info!("Starting WiFi scan...");
    let mut wifi_driver = WifiDriver::new(modem, sysloop, None).unwrap();
    wifi_driver
        .set_configuration(&Configuration::Client(ClientConfiguration::default()))
        .unwrap();
    wifi_driver.start().unwrap();

    log::info!("Scanning for WiFi networks...");
    let res = wifi_driver.scan().unwrap();
    log::info!("Scan complete. Found {} networks.", res.len());
    for network in res {
        log::info!("Found network: {:?}", network);
    }
    log::info!("WiFi scan finished.");
}
