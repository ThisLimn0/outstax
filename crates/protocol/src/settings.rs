//! Standalone printer settings, independently encoded from recovered wire layouts.
use crate::{err, Command, Frame, LinkModel, Result};

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum SleepMode {
    Current = 0,
    /// Never sent by any app; response unknown.
    Provisional = 1,
    Default = 2,
    ExtendCurrent = 3,
    /// Never sent by any app; response unknown.
    SetProvisional = 4,
    SetDefault = 5,
}
/// Four timer fields retain raw app units rather than guessing their meaning.
pub fn sleep_settings(mode: SleepMode, timers: [u16; 4]) -> Frame {
    let mut p = vec![mode as u8, 0, 0, 0];
    for timer in timers {
        p.extend_from_slice(&timer.to_be_bytes());
    }
    Frame::request(Command::SLEEP, p)
}
pub fn parse_sleep(frame: &Frame) -> Result<[u16; 4]> {
    if frame.command != Command::SLEEP {
        return Err(err("expected sleep settings"));
    }
    let p = frame.success_body()?;
    if p.len() < 8 {
        return Err(err("sleep reply requires four timers"));
    }
    Ok(std::array::from_fn(|i| {
        u16::from_be_bytes([p[i * 2], p[i * 2 + 1]])
    }))
}
/// The apps treat timer1 = 0 as auto-sleep turned off.
pub fn sleep_is_off(timers: [u16; 4]) -> bool {
    timers[0] == 0
}
/// App "auto-sleep off": write mode 5 with zero timers.
pub fn sleep_disable() -> Frame {
    sleep_settings(SleepMode::SetDefault, [0; 4])
}
/// App "auto-sleep on", step 1: read the default timers (mode 2).
pub fn sleep_enable_read() -> Frame {
    sleep_settings(SleepMode::Default, [0; 4])
}
/// App "auto-sleep on", step 2: write mode 5 with the timers from step 1.
pub fn sleep_enable(defaults: [u16; 4]) -> Frame {
    sleep_settings(SleepMode::SetDefault, defaults)
}
/// Wide/Square keep-alive: mode 3 with zero timers pushes back auto power-off.
pub fn sleep_extend() -> Frame {
    sleep_settings(SleepMode::ExtendCurrent, [0; 4])
}
/// Mode 1 reads and mode 5 writes; eleven trailing bytes are reserved. App
/// gates: Wide UI BO-22 only, Mini FI023/FI033, Square FI017.
pub fn power_led(model: LinkModel, write: bool, value: u8) -> Result<Frame> {
    if !matches!(
        model,
        LinkModel::Wide | LinkModel::Mini2 | LinkModel::Mini3 | LinkModel::Square
    ) {
        return Err(err("no app sends the power LED setting to this model"));
    }
    let mut p = vec![0; 16];
    p[0] = if write { 5 } else { 1 };
    p[4] = value;
    Ok(Frame::request(Command(0x30, 4), p))
}
/// Reply frame is at least 20 bytes; body[0] is the current value
/// (Wide: 0 rainbow, 1 warm, 2 cool, 3 white).
pub fn parse_power_led(frame: &Frame) -> Result<u8> {
    if frame.command != Command(0x30, 4) {
        return Err(err("expected power LED setting"));
    }
    let p = frame.success_body()?;
    if p.len() < 12 {
        return Err(err("power LED reply shorter than 20 bytes"));
    }
    Ok(p[0])
}
/// Square `X1.a`. Byte values come from a single-class fallback decompile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ButtonAction {
    NoAction = 0,
    Reprint = 1,
    Reserved = 2,
}
/// Square `W1.d` sends `[02, 01]` (mode int 3, set RE_PRINT) on every connect.
pub fn function_button_set(action: ButtonAction) -> Frame {
    Frame::request(Command(0x30, 8), vec![2, action as u8])
}
/// Reply frame is at least 9 bytes; any value other than 0/1 is RESERVED.
pub fn parse_function_button(frame: &Frame) -> Result<ButtonAction> {
    if frame.command != Command(0x30, 8) {
        return Err(err("expected function button setting"));
    }
    Ok(match frame.success_body()?.first() {
        None => return Err(err("function button reply shorter than 9 bytes")),
        Some(0) => ButtonAction::NoAction,
        Some(1) => ButtonAction::Reprint,
        Some(_) => ButtonAction::Reserved,
    })
}
pub fn axis_actions(power_off: bool, reprint: bool) -> Frame {
    Frame::request(
        Command(0x30, 2),
        vec![2, u8::from(power_off) | (u8::from(reprint) << 1)],
    )
}
pub fn ar_vibration(enabled: bool, intensity: u8) -> Frame {
    Frame::request(Command(0x30, 6), vec![u8::from(enabled), intensity, 0, 0])
}
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Parameter {
    ModeSwitch = 0,
    PrinterPower = 1,
    LedBrightness = 2,
    ShootingLight = 3,
    Other = 4,
}
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum ParameterMode {
    /// Never sent by the app; the read-response layout is unknown.
    Current = 0,
    /// Never sent by the app; the read-response layout is unknown.
    Factory = 1,
    Set = 2,
}
/// Mini `L1.m.C` sends this to Link 3 (FI033) only; Link+ is rejected locally.
/// Always SET mode; the reply is the result byte only. The app only ever sends
/// parameters 0 and 3.
pub fn parameter(model: LinkModel, parameter: Parameter, data: [u8; 8]) -> Result<Frame> {
    if model != LinkModel::Mini3 {
        return Err(err("the app sends 3020 parameters to mini Link 3 only"));
    }
    let mut p = vec![parameter as u8, ParameterMode::Set as u8];
    p.extend_from_slice(&data);
    Ok(Frame::request(Command(0x30, 0x20), p))
}
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum CameraMode {
    Normal = 0,
    AirStudio = 1,
    InstaxCamera = 2,
}
pub fn mode_switch(model: LinkModel, mode: CameraMode) -> Result<Frame> {
    let mut data = [0; 8];
    data[0] = mode as u8;
    parameter(model, Parameter::ModeSwitch, data)
}
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum ShootingLight {
    ThreeWhiteOn = 0,
    Off2s = 1,
}
/// The Mini app's connect step sends `ThreeWhiteOn`.
pub fn shooting_light(model: LinkModel, light: ShootingLight) -> Result<Frame> {
    let mut data = [0; 8];
    data[0] = light as u8;
    parameter(model, Parameter::ShootingLight, data)
}
/// Each step interleaves BGR for all LEDs: one LED for Link+, two for the older
/// double-pattern builder, three for Link 3. Callers must select the model layout.
pub fn led_pattern(
    command: Command,
    mode: u8,
    time_ms: u16,
    repeat: u8,
    steps: &[Vec<[u8; 3]>],
) -> Result<Frame> {
    if !matches!(command, Command(0x30, 1) | Command(0x30, 3))
        || mode > 3
        || steps.is_empty()
        || steps.len() > 255
        || time_ms / 10 > 255
    {
        return Err(err("invalid LED pattern header"));
    }
    let count = steps[0].len();
    if !(1..=3).contains(&count)
        || (command == Command::LED && count != 1)
        || steps.iter().any(|s| s.len() != count)
    {
        return Err(err("LED pattern has inconsistent channel count"));
    }
    let mut p = vec![mode, steps.len() as u8, (time_ms / 10) as u8, repeat];
    for step in steps {
        for [r, g, b] in step {
            p.extend_from_slice(&[*b, *g, *r]);
        }
    }
    Ok(Frame::request(command, p))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadCalibration {
    pub head_type: u8,
    pub judge: u8,
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub rgb: [u16; 3],
}
impl HeadCalibration {
    pub fn parse(frame: &Frame) -> Result<Self> {
        if frame.command != Command(0x30, 0x80) {
            return Err(err("expected head calibration"));
        }
        let p = frame.success_body()?;
        if p.len() < 13 {
            return Err(err("head calibration requires thirteen bytes"));
        }
        Ok(Self {
            head_type: p[0],
            judge: p[1],
            year: u16::from_be_bytes([p[3], p[4]]),
            month: p[5],
            day: p[6],
            rgb: std::array::from_fn(|i| u16::from_be_bytes([p[7 + i * 2], p[8 + i * 2]])),
        })
    }
    /// FUTABA's calendar-month correction in PrinterHeadLightCorrect. Only the
    /// original mini Link with qualifying firmware invokes this app path.
    pub fn correction(&self, year: u16, month: u8) -> Result<Option<[u16; 3]>> {
        if self.head_type != 0 || self.judge != 1 {
            return Ok(None);
        }
        if !(1..=12).contains(&month) || !(1..=12).contains(&self.month) {
            return Err(err("invalid calibration month"));
        }
        let h = [0.046, -0.0326, 0.0077];
        let a1 = [0.01, 0.0113, 0.0123];
        let a2 = [0.0043, 0.0051, 0.0054];
        let mut age = (i32::from(year) * 12 + i32::from(month)
            - i32::from(self.year) * 12
            - i32::from(self.month))
        .max(0);
        for c in 0..3 {
            if self.rgb[c] != 0 {
                let target = 63.0 / ((1.0 + h[c]) * f64::from(self.rgb[c])) - 1.0;
                let first = (target / a1[c]) as i32;
                let limit = if first <= 28 {
                    first
                } else {
                    ((target - 28.0 * a1[c]) / a2[c] + 28.0) as i32
                };
                age = age.min(limit).max(0);
            }
        }
        Ok(Some(std::array::from_fn(|c| {
            let factor =
                1.0 + a1[c] * f64::from(age.min(28)) + a2[c] * f64::from((age - 28).max(0));
            ((1.0 + h[c]) * f64::from(self.rgb[c]) * factor).clamp(0.0, 63.0) as u16
        })))
    }
}
pub fn set_head_correction(rgb: [u16; 3]) -> Result<Frame> {
    if rgb.iter().any(|c| *c > 63) {
        return Err(err("head correction outside 0..63"));
    }
    let mut p = Vec::with_capacity(6);
    for channel in rgb {
        p.extend_from_slice(&channel.to_be_bytes());
    }
    Ok(Frame::request(Command(0x30, 0x81), p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hex, unhex};
    fn wire(frame: &Frame) -> String {
        hex(&frame.encode().unwrap())
    }
    fn reply(text: &str) -> Frame {
        Frame::decode(&unhex(text).unwrap()).unwrap()
    }
    #[test]
    fn head_calibration_vectors() {
        assert_eq!(
            wire(&Frame::request(Command(0x30, 0x80), vec![])),
            "416200073080a5"
        );
        assert_eq!(
            wire(&set_head_correction([30, 31, 32]).unwrap()),
            "4162000d3081001e001f002041"
        );
    }
    #[test]
    fn parameter_vectors_and_gate() {
        let m = LinkModel::Mini3;
        let light = shooting_light(m, ShootingLight::ThreeWhiteOn).unwrap();
        assert_eq!(wire(&light), "41620011302003020000000000000000f6");
        let modes = [
            (CameraMode::Normal, "41620011302000020000000000000000f9"),
            (CameraMode::AirStudio, "41620011302000020100000000000000f8"),
            (
                CameraMode::InstaxCamera,
                "41620011302000020200000000000000f7",
            ),
        ];
        for (mode, expected) in modes {
            assert_eq!(wire(&mode_switch(m, mode).unwrap()), expected);
        }
        assert!(shooting_light(LinkModel::MiniPlus, ShootingLight::ThreeWhiteOn).is_err());
    }
    #[test]
    fn function_button_vectors() {
        assert_eq!(
            wire(&function_button_set(ButtonAction::Reprint)),
            "416200093008020118"
        );
        assert_eq!(
            parse_function_button(&reply("61 42 00 09 30 08 00 01 1a")).unwrap(),
            ButtonAction::Reprint
        );
    }
    #[test]
    fn power_led_vectors_and_gate() {
        let m = LinkModel::Wide;
        let pad = |n: usize| "00".repeat(n);
        assert_eq!(
            wire(&power_led(m, false, 0).unwrap()),
            format!("41620017300401{}10", pad(15))
        );
        assert_eq!(
            wire(&power_led(m, true, 3).unwrap()),
            format!("4162001730040500000003{}09", pad(11))
        );
        let led = reply(&format!("61 42 00 14 30 04 00 02 {} 12", pad(11)));
        assert_eq!(parse_power_led(&led).unwrap(), 2);
        assert!(power_led(LinkModel::Mini, false, 0).is_err());
    }
    #[test]
    fn sleep_vectors() {
        let frame = |mode: &str, sum: &str| format!("416200130102{mode}{}{sum}", "00".repeat(11));
        assert_eq!(
            wire(&sleep_settings(SleepMode::Current, [0; 4])),
            frame("00", "46")
        );
        assert_eq!(wire(&sleep_extend()), frame("03", "43"));
        assert_eq!(wire(&sleep_disable()), frame("05", "41"));
        assert_eq!(sleep_enable_read().payload[0], 2);
        assert_eq!(
            sleep_enable([180, 0, 0, 0]).payload,
            [5, 0, 0, 0, 0, 180, 0, 0, 0, 0, 0, 0]
        );
        let timers =
            parse_sleep(&reply("61 42 00 10 01 02 00 00 b4 00 00 00 00 00 00 95")).unwrap();
        assert_eq!(timers, [180, 0, 0, 0]);
        assert!(!sleep_is_off(timers) && sleep_is_off([0; 4]));
    }
    #[test]
    fn initialization_vectors() {
        let extra = |s| wire(&Frame::request(Command::ADDITIONAL_INFO, vec![s]));
        assert_eq!(extra(0), "4162000830100014");
        assert_eq!(extra(1), "4162000830100113");
        assert_eq!(wire(&axis_actions(true, true)), "41620009300202031c");
    }
}
