/// Send levels from each voice to each of 4 effect groups
pub struct RoutingMatrix {
    /// levels[voice][group] = send level (0.0-1.0)
    pub levels: [[f32; 4]; 16],
}

impl RoutingMatrix {
    pub fn new() -> Self {
        let mut levels = [[0.0_f32; 4]; 16];
        // Default: all voices send 100% to group A
        for i in 0..16 {
            levels[i][0] = 1.0;
        }
        RoutingMatrix { levels }
    }

    pub fn set(&mut self, voice: usize, group: usize, level: f32) {
        if voice < 16 && group < 4 {
            self.levels[voice][group] = level.clamp(0.0, 1.0);
        }
    }

    pub fn get(&self, voice: usize, group: usize) -> f32 {
        if voice < 16 && group < 4 {
            self.levels[voice][group]
        } else {
            0.0
        }
    }
}

impl Default for RoutingMatrix {
    fn default() -> Self {
        RoutingMatrix::new()
    }
}
