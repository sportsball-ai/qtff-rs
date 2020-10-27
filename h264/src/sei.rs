use super::{decode, sequence_parameter_set::VUIParameters, syntax_elements::*, Bitstream};

use std::io;

pub const SEI_PAYLOAD_TYPE_PIC_TIMING: u64 = 1;

// ITU-T H.264, 04/2017, 7.3.2.3.1
#[derive(Clone, Debug, Default)]
pub struct SEIMessage {
  pub payload_type: u64,
  pub payload_size: u64,
  pub pic_timing: Option<PicTiming>,
}

impl SEIMessage {
  pub fn decode<T: Iterator<Item = u8>>(bs: &mut Bitstream<T>, vui_params: &VUIParameters) -> io::Result<Self> {
    let mut ret = Self::default();

    let mut payload_type = 0;
    let mut payload_size = 0;
    let mut byte = bs.read_bits(8)?;
    while byte == 0xFF {
      payload_type += byte;
      byte = bs.read_bits(8)?;
    }
    payload_type += byte;
    ret.payload_type = payload_type;

    byte = bs.read_bits(8)?;
    while byte == 0xFF {
      payload_size += byte;
      byte = bs.read_bits(8)?;
    }
    payload_size += byte;
    ret.payload_size = payload_size;

    if payload_type == SEI_PAYLOAD_TYPE_PIC_TIMING {
      ret.pic_timing = Some(PicTiming::decode(bs, vui_params)?);
    }

    Ok(ret)
  }
}

#[derive(Clone, Debug, Default)]
pub struct PicTiming {
  pub cpb_removal_delay: u64,
  pub dpb_output_delay: u64,
  pub pic_struct: U4,
  pub timecodes: Vec<Timecode>,
}

#[derive(Clone, Debug, Default)]
pub struct Timecode {
  pub clock_timestamp_flag: U1,
  // if (clock_timestamp_flag) {
  pub ct_type: U2,
  pub nuit_field_based_flag: U1,
  pub counting_type: U5,
  pub full_timestamp_flag: U1,
  pub discontinuity_flag: U1,
  pub cnt_dropped_flag: U1,
  pub n_frames: U8,
  pub seconds: U6,
  pub minutes: U6,
  pub hours: U5,
  pub seconds_flag: U1,
  pub minutes_flag: U1,
  pub hours_flag: U1,
  // }
  pub time_offset: U32,
}

impl PicTiming {
  pub fn decode<T: Iterator<Item = u8>>(bs: &mut Bitstream<T>, vui_params: &VUIParameters) -> io::Result<Self> {
    let mut ret = Self::default();
    let mut hrd_params = None;
    if vui_params.cpb_dpb_delays_present_flag() {
      hrd_params = match (&vui_params.nal_hrd_parameters, &vui_params.vcl_hrd_parameters) {
        (Some(params), _) => Some(params),
        (_, Some(params)) => Some(params),
        _ => None,
      };

      if let Some(hrd_params) = hrd_params {
        ret.cpb_removal_delay = bs.read_bits(hrd_params.cpb_removal_delay_length_minus1.0 as usize + 1)?;
        ret.dpb_output_delay = bs.read_bits(hrd_params.dpb_output_delay_length_minus1.0 as usize + 1)?;
      }
    }

    if vui_params.pic_struct_present_flag.0 == 0 {
      return Ok(ret);
    }

    decode!(bs, &mut ret.pic_struct)?;

    for _ in 0..ret.num_clock_ts() {
      let mut timecode = Timecode::default();

      decode!(bs, &mut timecode.clock_timestamp_flag)?;
      if timecode.clock_timestamp_flag.0 == 0 {
        continue;
      }

      decode!(
        bs,
        &mut timecode.ct_type,
        &mut timecode.nuit_field_based_flag,
        &mut timecode.counting_type,
        &mut timecode.full_timestamp_flag,
        &mut timecode.discontinuity_flag,
        &mut timecode.cnt_dropped_flag,
        &mut timecode.n_frames
      )?;

      if timecode.full_timestamp_flag.0 == 1 {
        decode!(bs, &mut timecode.seconds, &mut timecode.minutes, &mut timecode.hours)?;
      } else {
        decode!(bs, &mut timecode.seconds_flag)?;
        if timecode.seconds_flag.0 == 1 {
          decode!(bs, &mut timecode.seconds, &mut timecode.minutes_flag)?;
          if timecode.minutes_flag.0 == 1 {
            decode!(bs, &mut timecode.minutes, &mut timecode.hours_flag)?;
            if timecode.hours_flag.0 == 1 {
              decode!(bs, &mut timecode.hours)?;
            }
          }
        }
      }

      let time_offset_length = match hrd_params {
        Some(hrd_params) if hrd_params.time_offset_length.0 > 0 => hrd_params.time_offset_length.0 as usize,
        _ => 24,
      };

      timecode.time_offset.0 = bs.read_bits(time_offset_length)? as u32;
      ret.timecodes.push(timecode);
    }

    Ok(ret)
  }

  pub fn num_clock_ts(&self) -> usize {
    match self.pic_struct.0 {
      0..=2 => 1,
      3..=6 => 2,
      _ => 3,
    }
  }
}
