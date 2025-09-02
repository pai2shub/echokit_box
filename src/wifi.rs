use esp_idf_svc::wifi::ClientConfiguration;
use esp_idf_svc::wifi::Configuration;
use esp_idf_svc::wifi::WifiDriver;

pub fn scan(
    modem: impl esp_idf_svc::hal::peripheral::Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
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

pub fn connect(
    ssid: &str,
    pass: &str,
    modem: impl esp_idf_svc::hal::peripheral::Peripheral<P = esp_idf_svc::hal::modem::Modem> + 'static,
    sysloop: esp_idf_svc::eventloop::EspSystemEventLoop,
) -> anyhow::Result<Box<esp_idf_svc::wifi::EspWifi<'static>>> {
    let mut auth_method = esp_idf_svc::wifi::AuthMethod::WPA2Personal;
    if ssid.is_empty() {
        anyhow::bail!("Missing WiFi name")
    }
    if pass.is_empty() {
        auth_method = esp_idf_svc::wifi::AuthMethod::None;
        log::info!("Wifi password is empty");
    }
    let mut esp_wifi = esp_idf_svc::wifi::EspWifi::new(modem, sysloop.clone(), None)?;

    let mut wifi = esp_idf_svc::wifi::BlockingWifi::wrap(&mut esp_wifi, sysloop)?;

    wifi.set_configuration(&esp_idf_svc::wifi::Configuration::Client(
        esp_idf_svc::wifi::ClientConfiguration {
            ssid: ssid
                .try_into()
                .expect("Could not parse the given SSID into WiFi config"),
            password: pass
                .try_into()
                .expect("Could not parse the given password into WiFi config"),
            auth_method,
            ..Default::default()
        },
    ))?;

    wifi.start()?;

    log::info!("Connecting wifi...");

    wifi.connect()?;

    log::info!("Waiting for DHCP lease...");

    wifi.wait_netif_up()?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    log::info!("Wifi DHCP info: {:?}", ip_info);

    Ok(Box::new(esp_wifi))
}
