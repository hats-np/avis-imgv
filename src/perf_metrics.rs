use eframe::egui;
use std::time::Instant;

pub struct PerfMetrics {
    start_of_frame: Instant,
    longest_frametime: u128,
    longest_recent_frametime: u128,
    current_frametime: u128,
    current_frametime_micros: u128,
}

impl Default for PerfMetrics {
    fn default() -> PerfMetrics {
        Self::new()
    }
}

impl PerfMetrics {
    pub fn new() -> PerfMetrics {
        PerfMetrics {
            start_of_frame: Instant::now(),
            longest_frametime: 0,
            longest_recent_frametime: 0,
            current_frametime: 0,
            current_frametime_micros: 0,
        }
    }

    pub fn new_frame(&mut self) {
        self.start_of_frame = Instant::now()
    }

    pub fn end_frame(&mut self) {
        self.current_frametime = self.start_of_frame.elapsed().as_millis();
        self.current_frametime_micros = self.start_of_frame.elapsed().as_micros();

        if self.current_frametime > self.longest_frametime {
            self.longest_frametime = self.current_frametime;
        }

        if self.current_frametime > 0 {
            self.longest_recent_frametime = self.current_frametime;
        }
    }

    pub fn display_metrics(&mut self, ui: &mut egui::Ui) {
        ui.monospace(format!(
            "Current: {}mils • {}mics | Recent: {}mils | Longest: {}mils",
            self.current_frametime,
            self.current_frametime_micros,
            self.longest_recent_frametime,
            self.longest_frametime
        ));

        println!("{}", self.current_frametime);
    }
}
