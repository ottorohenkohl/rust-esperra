use heapless::Vec;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BluetoothConnectionState {
    AlreadyConnected,
    ConnectionEstablished,
    ConnectionFailed,
}


#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BluetoothCommand {
    Connect(BluetoothProfile),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BluetoothNotification {
    pub characteristic_uuid: u16,
    pub payload: Vec<u8, 64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BluetoothProfile {
    pub advertisement_name_prefix: &'static str,
    pub advertisement_service_uuid: u16,
    pub service_uuid: u16,
    pub notification_uuid: u16,
}
