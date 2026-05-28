pub struct Pid {
    p: f32,
    i: f32,
    d: f32,
    setpoint: f32,
    prev_error: f32,
    integral: f32,
    integral_max:f32
}

impl Pid {
    pub const fn new(p: f32, i: f32, d: f32, setpoint: f32,integral_max:f32) -> Self {
        Self {
            p,
            i,
            d,
            setpoint,
            prev_error: 0f32,
            integral: 0f32,
            integral_max
        }
    }
    pub fn run(&mut self, is: f32, delta_t: f32) -> f32 {
        let error = self.setpoint - is;
        let mut output = error * self.p;
        output += ((error - self.prev_error) / delta_t) * self.d;
        self.prev_error = error;
        //clamping output to prevent wind-u
        self.integral = error.mul_add(delta_t, self.integral);
        self.integral = self.integral.clamp(-self.integral_max,self.integral_max);

        output = self.integral.mul_add(self.i, output);
        output.clamp(0.0, 1.0f32)
    }
}
