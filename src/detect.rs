//! STA/LTA P-wave detection (shared Rust core).
//!
//! Mirrors the Python/ObsPy spec the country adapters use (recursive STA/LTA)
//! so picks agree across implementations. Reused by the node and, via
//! flutter_rust_bridge, by the mobile on-device detector (v1.1). Deterministic
//! by design — no ML in the alert path (DESIGN guardrail).
//!
//! Pipeline (matches the Python adapter spec): band-pass 2–8 Hz → recursive
//! STA/LTA → trigger. The band-pass drops long-period noise (it halved false
//! positives in backtesting). The filter is a 4th-order Butterworth applied
//! zero-phase (forward+reverse), equivalent to ObsPy's `bandpass(...,
//! zerophase=True)`.

/// Band-pass low cut (Hz).
pub const FREQ_MIN: f64 = 2.0;
/// Band-pass high cut (Hz).
pub const FREQ_MAX: f64 = 8.0;
/// Butterworth prototype order (ObsPy `corners`).
pub const FILTER_CORNERS: usize = 4;
/// Short-term-average window (seconds). Keep in sync with the Python adapter.
pub const STA_SECONDS: f64 = 0.5;
/// Long-term-average window (seconds).
pub const LTA_SECONDS: f64 = 10.0;
/// STA/LTA ratio that declares a pick.
pub const TRIGGER_ON: f64 = 6.0;
/// Ratio below which a trigger is considered over.
pub const TRIGGER_OFF: f64 = 1.5;

/// A detected P-wave onset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pick {
    /// Sample index of the onset.
    pub index: usize,
    /// STA/LTA ratio at the onset.
    pub sta_lta_ratio: f64,
}

/// Recursive STA/LTA characteristic function (matches ObsPy `recursive_sta_lta`).
/// The first `nlta` samples are zeroed (warm-up).
pub fn recursive_sta_lta(samples: &[f64], nsta: usize, nlta: usize) -> Vec<f64> {
    let mut cft = vec![0.0; samples.len()];
    if samples.is_empty() {
        return cft;
    }
    let csta = 1.0 / nsta as f64;
    let clta = 1.0 / nlta as f64;
    let icsta = 1.0 - csta;
    let iclta = 1.0 - clta;
    let mut sta = 0.0;
    let mut lta = 1e-99;
    for i in 1..samples.len() {
        let sq = samples[i] * samples[i];
        sta = csta * sq + icsta * sta;
        lta = clta * sq + iclta * lta;
        cft[i] = sta / lta;
    }
    for c in cft.iter_mut().take(nlta.min(samples.len())) {
        *c = 0.0;
    }
    cft
}

/// Minimal complex number for Butterworth filter design.
#[derive(Clone, Copy)]
struct Cf {
    re: f64,
    im: f64,
}

impl Cf {
    fn new(re: f64, im: f64) -> Cf {
        Cf { re, im }
    }
    fn add(self, o: Cf) -> Cf {
        Cf::new(self.re + o.re, self.im + o.im)
    }
    fn sub(self, o: Cf) -> Cf {
        Cf::new(self.re - o.re, self.im - o.im)
    }
    fn mul(self, o: Cf) -> Cf {
        Cf::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
    fn div(self, o: Cf) -> Cf {
        let d = o.re * o.re + o.im * o.im;
        Cf::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
    fn scale(self, s: f64) -> Cf {
        Cf::new(self.re * s, self.im * s)
    }
    /// Principal complex square root.
    fn sqrt(self) -> Cf {
        let r = (self.re * self.re + self.im * self.im).sqrt();
        let re = ((r + self.re) / 2.0).sqrt();
        let im = ((r - self.re) / 2.0).sqrt();
        Cf::new(re, if self.im < 0.0 { -im } else { im })
    }
}

/// Designs Butterworth band-pass second-order sections `[b0,b1,b2,a1,a2]`
/// (a0 = 1), mirroring scipy/ObsPy's zpk pipeline (buttap → lp2bp → bilinear).
fn butter_bandpass_sos(freqmin: f64, freqmax: f64, fs: f64, corners: usize) -> Vec<[f64; 5]> {
    let nyq = 0.5 * fs;
    // pre-warp frequencies (scipy uses an internal fs = 2)
    let warped_lo = 4.0 * (std::f64::consts::PI * (freqmin / nyq) / 2.0).tan();
    let warped_hi = 4.0 * (std::f64::consts::PI * (freqmax / nyq) / 2.0).tan();
    let bw = warped_hi - warped_lo;
    let wo2 = Cf::new(warped_lo * warped_hi, 0.0); // band center squared

    // analog Butterworth lowpass prototype poles: p_k = -exp(i*pi*m/(2N))
    let n = corners;
    let mut lp: Vec<Cf> = Vec::with_capacity(n);
    let mut m = -(n as i64) + 1;
    while m < n as i64 {
        let ang = std::f64::consts::PI * (m as f64) / (2.0 * n as f64);
        lp.push(Cf::new(-ang.cos(), -ang.sin()));
        m += 2;
    }

    // lp2bp: each pole -> two band-pass poles  p_lp ± sqrt(p_lp^2 - wo^2)
    let mut poles: Vec<Cf> = Vec::with_capacity(2 * n);
    for p in &lp {
        let plp = p.scale(bw / 2.0);
        let disc = plp.mul(plp).sub(wo2).sqrt();
        poles.push(plp.add(disc));
        poles.push(plp.sub(disc));
    }

    // bilinear transform: z = (fs2 + s)/(fs2 - s), fs2 = 4
    let fs2 = Cf::new(4.0, 0.0);
    let dpoles: Vec<Cf> = poles.iter().map(|&s| fs2.add(s).div(fs2.sub(s))).collect();

    // gain k = bw^n * real( prod(fs2 - 0)/prod(fs2 - p_bp) ), zeros at s=0
    let mut den = Cf::new(1.0, 0.0);
    for &s in &poles {
        den = den.mul(fs2.sub(s));
    }
    let num = Cf::new(bw.powi(n as i32) * 4.0_f64.powi(n as i32), 0.0);
    let k = num.div(den).re;

    // pair conjugate poles; band-pass numerator (z-1)(z+1) -> b = [1,0,-1]
    let mut sos: Vec<[f64; 5]> = Vec::with_capacity(n);
    for &p in dpoles.iter().filter(|p| p.im > 1e-12) {
        sos.push([1.0, 0.0, -1.0, -2.0 * p.re, p.re * p.re + p.im * p.im]);
    }
    debug_assert_eq!(sos.len(), n);
    if let Some(first) = sos.first_mut() {
        first[0] = k;
        first[2] = -k;
    }
    sos
}

/// Applies SOS sections in series (transposed direct form II).
fn sosfilt(sos: &[[f64; 5]], x: &[f64]) -> Vec<f64> {
    let mut data = x.to_vec();
    for s in sos {
        let (b0, b1, b2, a1, a2) = (s[0], s[1], s[2], s[3], s[4]);
        let (mut z1, mut z2) = (0.0, 0.0);
        for v in data.iter_mut() {
            let xn = *v;
            let yn = b0 * xn + z1;
            z1 = b1 * xn - a1 * yn + z2;
            z2 = b2 * xn - a2 * yn;
            *v = yn;
        }
    }
    data
}

/// Zero-phase band-pass (forward then reverse pass) — ObsPy `zerophase=True`.
pub fn bandpass(samples: &[f64], freqmin: f64, freqmax: f64, fs: f64, corners: usize) -> Vec<f64> {
    if samples.is_empty() {
        return Vec::new();
    }
    let sos = butter_bandpass_sos(freqmin, freqmax, fs, corners);
    let fwd = sosfilt(&sos, samples);
    let rev: Vec<f64> = fwd.into_iter().rev().collect();
    let mut out = sosfilt(&sos, &rev);
    out.reverse();
    out
}

/// Returns the first P-wave pick in `samples`, or `None`.
///
/// `samples` is one channel; `sampling_rate` in Hz. Mirrors the adapter's
/// `detect_pick`: band-pass 2–8 Hz, recursive STA/LTA, first crossing above
/// [`TRIGGER_ON`]. Needs more than `LTA_SECONDS` of data.
pub fn detect_pick(samples: &[f64], sampling_rate: f64) -> Option<Pick> {
    if sampling_rate <= 0.0 || samples.is_empty() {
        return None;
    }
    let nsta = ((STA_SECONDS * sampling_rate) as usize).max(1);
    let nlta = ((LTA_SECONDS * sampling_rate) as usize).max(nsta + 1);
    if samples.len() <= nlta {
        return None;
    }
    let filtered = bandpass(samples, FREQ_MIN, FREQ_MAX, sampling_rate, FILTER_CORNERS);
    let cft = recursive_sta_lta(&filtered, nsta, nlta);
    cft.iter()
        .enumerate()
        .find(|(_, &r)| r >= TRIGGER_ON)
        .map(|(index, &sta_lta_ratio)| Pick {
            index,
            sta_lta_ratio,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: f64, n: usize, hz: f64, amp: f64) -> Vec<f64> {
        let w = 2.0 * std::f64::consts::PI * hz / rate;
        (0..n).map(|i| amp * (w * i as f64).sin()).collect()
    }

    /// Out-of-band low-freq drift baseline, then an in-band P-like burst.
    fn signal(rate: f64, total_s: f64, burst_start_s: f64, burst_len_s: f64) -> Vec<f64> {
        let n = (rate * total_s) as usize;
        let b0 = (rate * burst_start_s) as usize;
        let b1 = b0 + (rate * burst_len_s) as usize;
        let drift = 2.0 * std::f64::consts::PI * 0.4 / rate; // 0.4 Hz (out of band)
        let burst = 2.0 * std::f64::consts::PI * 5.0 / rate; // 5 Hz (in band)
        (0..n)
            .map(|i| {
                let base = 0.3 * (drift * i as f64).sin();
                if i >= b0 && i < b1 {
                    base + 6.0 * (burst * i as f64).sin()
                } else {
                    base
                }
            })
            .collect()
    }

    #[test]
    fn detects_inband_burst_onset() {
        let rate = 100.0;
        let s = signal(rate, 20.0, 15.0, 2.0);
        let pick = detect_pick(&s, rate).expect("should detect the in-band burst");
        let onset = (15.0 * rate) as usize;
        assert!(
            (pick.index as i64 - onset as i64).abs() < (2.0 * rate) as i64,
            "onset off: got {}, expected ~{}",
            pick.index,
            onset
        );
        assert!(pick.sta_lta_ratio >= TRIGGER_ON);
    }

    #[test]
    fn bandpass_rejects_lowfreq_drift() {
        // A large 0.4 Hz drift would trip raw STA/LTA; the band-pass removes it.
        let rate = 100.0;
        let drift = sine(rate, (20.0 * rate) as usize, 0.4, 8.0);
        assert!(detect_pick(&drift, rate).is_none());
    }

    #[test]
    fn bandpass_keeps_inband_attenuates_out_of_band() {
        let rate = 100.0;
        let n = 2000;
        let peak = |v: &[f64]| {
            v[200..v.len() - 200]
                .iter()
                .fold(0.0f64, |m, &x| m.max(x.abs()))
        };
        let fi = bandpass(
            &sine(rate, n, 5.0, 1.0),
            FREQ_MIN,
            FREQ_MAX,
            rate,
            FILTER_CORNERS,
        );
        let fo = bandpass(
            &sine(rate, n, 0.4, 1.0),
            FREQ_MIN,
            FREQ_MAX,
            rate,
            FILTER_CORNERS,
        );
        assert!(
            peak(&fi) > 0.7,
            "in-band attenuated too much: {}",
            peak(&fi)
        );
        assert!(peak(&fo) < 0.1, "out-of-band not attenuated: {}", peak(&fo));
    }

    #[test]
    fn no_pick_on_quiet_signal() {
        let rate = 100.0;
        let s = sine(rate, (20.0 * rate) as usize, 0.4, 0.2);
        assert!(detect_pick(&s, rate).is_none());
    }

    #[test]
    fn bandpass_matches_obspy_reference() {
        // Cross-checked against ObsPy bandpass(corners=4, zerophase=True): the
        // outputs agree to 6 decimals (see scratchpad/parity_obspy.py).
        let rate = 100.0;
        let x: Vec<f64> = (0..1000)
            .map(|i| {
                let t = i as f64 / rate;
                (2.0 * std::f64::consts::PI * 5.0 * t).sin()
                    + 0.5 * (2.0 * std::f64::consts::PI * 0.5 * t).sin()
            })
            .collect();
        let y = bandpass(&x, 2.0, 8.0, rate, 4);
        let round6 = |v: f64| (v * 1e6).round() / 1e6;
        assert_eq!(round6(y[501]), 0.309001);
        assert_eq!(round6(y[503]), 0.808975);
    }

    #[test]
    fn too_short_returns_none() {
        assert!(detect_pick(&[0.0; 10], 100.0).is_none());
    }

    #[test]
    fn invalid_rate_returns_none() {
        assert!(detect_pick(&[1.0; 5000], 0.0).is_none());
    }
}
