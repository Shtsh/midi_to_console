use std::convert::TryFrom;
use std::error::Error;
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use log::{info, trace};

use midir::{Ignore, MidiInput};

/// This thread has infinite loop in the end to process midi forever
pub fn process_signals(position: usize, tx: Sender<MidiMessageData>) -> Result<(), Box<dyn Error>> {
    let mut midi_in = MidiInput::new("midir reading input")?;
    midi_in.ignore(Ignore::None);

    let in_ports = midi_in.ports();

    let in_port = match in_ports.len() {
        0 => return Err("no input port found".into()),
        1 => {
            info!(
                "Choosing the only available input port: {}",
                midi_in.port_name(&in_ports[0]).unwrap()
            );
            &in_ports[0]
        }
        _ => {
            info!("Available input ports:");
            for (i, p) in in_ports.iter().enumerate() {
                info!("{}: {}", i, midi_in.port_name(p).unwrap());
            }
            in_ports
                .get(position)
                .ok_or("invalid MIDI input port selected")?
        }
    };

    let in_port_name = midi_in.port_name(in_port)?;
    info!("Connecting to {}", in_port_name);

    // _conn_in needs to be a named parameter, because it needs to be kept alive until the end of the scope
    let _conn_in = midi_in.connect(
        in_port,
        "midir-read-input",
        move |_, message: &[u8], _| {
            let midi_data = MidiMessageData::new(message[0], message[1], message[2]).unwrap();
            if midi_data.status_byte == MidiMessageTypes::NoteOn {
                trace!("tx_midi <- {:#04X?}", midi_data.data_byte1);
                // sending the midi data several times
                // to modify several reports as adding to one InputReport is not enough
                // for game to detect the hit
                tx.send(midi_data.clone()).unwrap();
                tx.send(midi_data).unwrap();
            }
        },
        (),
    )?;

    loop {
        thread::sleep(Duration::from_millis(1));
    }
}

/// Structure to store MIDI data packet
/// Packet consists of 3 bytes
///     0 - Status byte + channel
///     1 - Data byte 1
///     2 - Data byte 2
///```
/// Voice Message           Status Byte      Data Byte1          Data Byte2
/// -------------           -----------   -----------------   -----------------
/// Note off                      8x      Key number          Note Off velocity
/// Note on                       9x      Key number          Note on velocity
/// Polyphonic Key Pressure       Ax      Key number          Amount of pressure
/// Control Change                Bx      Controller number   Controller value
/// Program Change                Cx      Program number      None
/// Channel Pressure              Dx      Pressure value      None
/// Pitch Bend                    Ex      MSB                 LSB
/// ```

#[derive(Clone)]
pub struct MidiMessageData {
    pub channel: u8,
    pub status_byte: MidiMessageTypes,
    pub data_byte1: u8,
    pub data_byte2: u8,
}

impl MidiMessageData {
    fn new(byte0: u8, byte1: u8, byte2: u8) -> Result<MidiMessageData, Box<dyn Error>> {
        let midi_type = match MidiMessageTypes::try_from(byte0 >> 4) {
            Ok(v) => v,
            Err(_) => return Err("Incorrect MidiMessageType".into()),
        };
        Ok(MidiMessageData {
            channel: byte0 & 0x0F,
            status_byte: midi_type,
            data_byte1: byte1,
            data_byte2: byte2,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum MidiMessageTypes {
    NoteOff = 0x8,
    NoteOn = 0x9,
    PolyphonicPressure = 0xA,
    ControlChange = 0xB,
    ProgramChange = 0xC,
    ChannelPressure = 0xD,
    PitchBend = 0xE,
}

impl TryFrom<u8> for MidiMessageTypes {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == MidiMessageTypes::NoteOff as u8 => Ok(MidiMessageTypes::NoteOff),
            x if x == MidiMessageTypes::NoteOn as u8 => Ok(MidiMessageTypes::NoteOn),
            x if x == MidiMessageTypes::PolyphonicPressure as u8 => {
                Ok(MidiMessageTypes::PolyphonicPressure)
            }
            x if x == MidiMessageTypes::ControlChange as u8 => Ok(MidiMessageTypes::ControlChange),
            x if x == MidiMessageTypes::ProgramChange as u8 => Ok(MidiMessageTypes::ProgramChange),
            x if x == MidiMessageTypes::ChannelPressure as u8 => {
                Ok(MidiMessageTypes::ChannelPressure)
            }
            x if x == MidiMessageTypes::PitchBend as u8 => Ok(MidiMessageTypes::PitchBend),
            _ => Err(()),
        }
    }
}
