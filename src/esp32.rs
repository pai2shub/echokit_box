use std::rc::Rc;
use std::sync::{Arc, Mutex};

use esp_idf_svc::sys::EspError;

const DISPLAY_WIDTH: usize = 240;
const DISPLAY_HEIGHT: usize = 240;

fn init_spi() -> Result<(), EspError> {
    use esp_idf_svc::sys::*;
    const GPIO_NUM_NC: i32 = -1;
    const DISPLAY_MOSI_PIN: i32 = 47;
    const DISPLAY_CLK_PIN: i32 = 21;
    let mut buscfg = spi_bus_config_t::default();
    buscfg.__bindgen_anon_1.mosi_io_num = DISPLAY_MOSI_PIN;
    buscfg.__bindgen_anon_2.miso_io_num = GPIO_NUM_NC;
    buscfg.sclk_io_num = DISPLAY_CLK_PIN;
    buscfg.__bindgen_anon_3.quadwp_io_num = GPIO_NUM_NC;
    buscfg.__bindgen_anon_4.quadhd_io_num = GPIO_NUM_NC;
    buscfg.max_transfer_sz = (DISPLAY_WIDTH * DISPLAY_HEIGHT * std::mem::size_of::<u16>()) as i32;
    esp!(unsafe {
        spi_bus_initialize(
            spi_host_device_t_SPI3_HOST,
            &buscfg,
            spi_common_dma_t_SPI_DMA_CH_AUTO,
        )
    })
}

static mut ESP_LCD_PANEL_HANDLE: esp_idf_svc::sys::esp_lcd_panel_handle_t = std::ptr::null_mut();

fn init_lcd() -> Result<(), EspError> {
    use esp_idf_svc::sys::*;
    const DISPLAY_CS_PIN: i32 = 41;
    const DISPLAY_DC_PIN: i32 = 40;
    ::log::info!("Install panel IO");
    let mut panel_io: esp_lcd_panel_io_handle_t = std::ptr::null_mut();
    let mut io_config = esp_lcd_panel_io_spi_config_t::default();
    io_config.cs_gpio_num = DISPLAY_CS_PIN;
    io_config.dc_gpio_num = DISPLAY_DC_PIN;
    io_config.spi_mode = 3;
    io_config.pclk_hz = 40 * 1000 * 1000;
    io_config.trans_queue_depth = 10;
    io_config.lcd_cmd_bits = 8;
    io_config.lcd_param_bits = 8;
    esp!(unsafe {
        esp_lcd_new_panel_io_spi(spi_host_device_t_SPI3_HOST as _, &io_config, &mut panel_io)
    })?;

    ::log::info!("Install LCD driver");
    const DISPLAY_RST_PIN: i32 = 45;
    let mut panel_config = esp_lcd_panel_dev_config_t::default();
    let mut panel: esp_lcd_panel_handle_t = std::ptr::null_mut();

    panel_config.reset_gpio_num = DISPLAY_RST_PIN;
    panel_config.data_endian = lcd_rgb_data_endian_t_LCD_RGB_DATA_ENDIAN_LITTLE;
    panel_config.__bindgen_anon_1.rgb_ele_order = lcd_rgb_element_order_t_LCD_RGB_ELEMENT_ORDER_RGB;
    panel_config.bits_per_pixel = 16;

    esp!(unsafe { esp_lcd_new_panel_st7789(panel_io, &panel_config, &mut panel) })?;
    unsafe { ESP_LCD_PANEL_HANDLE = panel };

    const DISPLAY_MIRROR_X: bool = true;
    const DISPLAY_MIRROR_Y: bool = false;
    const DISPLAY_SWAP_XY: bool = false;
    const DISPLAY_INVERT_COLOR: bool = true;

    ::log::info!("Reset LCD panel");
    unsafe {
        esp!(esp_lcd_panel_reset(panel))?;
        esp!(esp_lcd_panel_init(panel))?;
        esp!(esp_lcd_panel_invert_color(panel, DISPLAY_INVERT_COLOR))?;
        esp!(esp_lcd_panel_swap_xy(panel, DISPLAY_SWAP_XY))?;
        esp!(esp_lcd_panel_mirror(
            panel,
            DISPLAY_MIRROR_X,
            DISPLAY_MIRROR_Y
        ))?;
        esp!(esp_lcd_panel_disp_on_off(panel, true))?; /* 启动屏幕 */
    }
    ::log::info!("LCD panel initialized successfully");
    Ok(())
}

struct EspPlatform {
    panel_handle: esp_idf_svc::sys::esp_lcd_panel_handle_t,
    window: Rc<slint::platform::software_renderer::MinimalSoftwareWindow>,
    timer: esp_idf_svc::timer::EspTimerService<esp_idf_svc::timer::Task>,
    queue: Arc<Mutex<Vec<Event>>>,
}

impl EspPlatform {
    pub fn new() -> std::boxed::Box<Self> {
        init_spi().unwrap();
        init_lcd().unwrap();

        log::info!("ESP32 Slint platform initialized");

        // Setup the window
        let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
            slint::platform::software_renderer::RepaintBufferType::SwappedBuffers,
        );
        log::info!(
            "Creating window with size {}x{}",
            DISPLAY_WIDTH,
            DISPLAY_HEIGHT
        );
        window.set_size(slint::PhysicalSize::new(
            DISPLAY_WIDTH as u32,
            DISPLAY_HEIGHT as u32,
        ));
        log::info!("Window created");

        std::boxed::Box::new(Self {
            panel_handle: unsafe { ESP_LCD_PANEL_HANDLE },
            window,
            timer: esp_idf_svc::timer::EspTimerService::new().unwrap(),
            queue: Default::default(),
        })
    }
}

impl slint::platform::Platform for EspPlatform {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        // Since on MCUs, there can be only one window, just return a clone of self.window.
        // We'll also use the same window in the event loop.
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> core::time::Duration {
        self.timer.now()
    }
    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        // Create a buffer to draw the scene
        log::info!("Starting event loop");

        use slint::platform::software_renderer::Rgb565Pixel;

        // 在这里手动创建两个帧缓冲区，用于双缓冲。
        log::info!("Creating frame buffer1");
        let mut buffer1 = vec![Rgb565Pixel::default(); DISPLAY_WIDTH * DISPLAY_HEIGHT];
        // log::info!("Creating frame buffer2");
        // let mut buffer2 = vec![Rgb565Pixel::default(); DISPLAY_WIDTH * DISPLAY_HEIGHT];
        log::info!("Entering main loop");

        loop {
            log::info!("Processing events");

            slint::platform::update_timers_and_animations();

            let queue = std::mem::take(&mut *self.queue.lock().unwrap());
            for event in queue {
                match event {
                    Event::Invoke(event) => event(),
                    Event::Quit => break,
                }
            }

            // Draw the scene if something needs to be drawn.
            self.window.draw_if_needed(|renderer| {
                log::info!("Drawing the scene");
                renderer.render(&mut buffer1, DISPLAY_WIDTH);
                log::info!("Scene drawn, flushing to display");
                unsafe {
                    let e = esp_idf_svc::sys::esp_lcd_panel_draw_bitmap(
                        self.panel_handle,
                        0,
                        0,
                        DISPLAY_WIDTH as i32,
                        DISPLAY_HEIGHT as i32,
                        buffer1.as_ptr().cast(),
                    );
                    if e != 0 {
                        log::warn!("flush_display error: {}", e);
                    }
                    log::info!("flush_display drawn to display");
                };
                log::info!("Swapping buffers");
                // core::mem::swap(&mut buffer1, &mut buffer2);
            });

            log::info!("Drawing completed, checking for active animations");

            // Try to put the MCU to sleep
            if !self.window.has_active_animations() {
                log::info!("No active animations, putting MCU to sleep");
                continue;
            }

            log::info!("Active animations, yielding to the scheduler");
            // FIXME
            esp_idf_svc::hal::task::do_yield();
        }
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        log::debug!("{}", arguments);
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn slint::platform::EventLoopProxy>> {
        Some(Box::new(EspEventLoopProxy {
            queue: self.queue.clone(),
        }))
    }
}

enum Event {
    Quit,
    Invoke(Box<dyn FnOnce() + Send>),
}
struct EspEventLoopProxy {
    queue: Arc<Mutex<Vec<Event>>>,
}
impl slint::platform::EventLoopProxy for EspEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        self.queue.lock().unwrap().push(Event::Quit);
        Ok(())
    }

    fn invoke_from_event_loop(
        &self,
        event: Box<dyn FnOnce() + Send>,
    ) -> Result<(), slint::EventLoopError> {
        self.queue.lock().unwrap().push(Event::Invoke(event));
        Ok(())
    }
}

pub fn init() {
    slint::platform::set_platform(EspPlatform::new()).unwrap();
}
