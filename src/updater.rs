#[warn(unreachable_code)]
extern crate serialport;

use std::{
    time::{Duration, Instant},
    fs,
};

use serialport::{*};

use cellaxon_base::tool::ticker::Ticker;

use e_drone::file::EncryptedBinary;
use e_drone::communication::{*};
use e_drone::communication::receiver::{*};
use e_drone::system::{*};
use e_drone::protocol::{*};
use e_drone::{*};


pub enum Sequence
{
    LookUpNewDevice,            // 새로 연결된 장치를 검색하고 새로운 장치가 있는 경우 연결
    CheckDeviceInformation,     // 연결된 장치의 정보 요청
    CheckUpdateLocation,        // 연결된 장치의 업데이트 정보 요청
    FirmwareUpdate,             // 펌웨어 업데이트
    UpdateComplete,             // 업데이트 완료 처리

    // Error State
    NoFirmwareFile,     // 펌웨어 파일이 없음
    NoAnswer,           // 장치로부터 응답이 없음
    NoMatchedFirmwareFile,      // 장치와 일치하는 펌웨어 파일이 없음
    ApplicationMode,    // 장치가 애플리케이션 모드로 동작 중(부트로더 모드로 다시 연결 필요)
    UpdateTimeOver,     // 업데이트 제한 시간 초과
}


pub struct Updater
{
    sequence: Sequence,

    vec_file: Vec<EncryptedBinary>,
    vec_serialport: Vec<String>,    // 시작 시 시리얼포트 목록

    ticker_transfer: Ticker,

    port: Result<Box<dyn SerialPort>>,
    buffer: [u8; 1024],
    receiver: Receiver,
    data: Data,

    device_type_target: DeviceType,
    information_target: Information,
    update_location: UpdateLocation,
    index_target_vec_file: usize,
    index_block_end: u16,
    message_status: String,
    message_version: String,
    update: Update,
    flag_transfer_success: bool,
    count_error: u32,

    flag_show_debug_message: bool,  // 디버깅 정보 표시
    
    time_transfer: Instant,
    time_receive: Instant,

    time_sequence_start: Instant,
}


impl Updater
{
    pub fn new() -> Updater 
    {
        let mut updater = Updater
        {
            sequence: Sequence::LookUpNewDevice,

            vec_file: Updater::lookup_firmware_files(),
            vec_serialport: Vec::new(),

            ticker_transfer: Ticker::new(200),

            port: Err(serialport::Error{kind: serialport::ErrorKind::NoDevice, description: "Not Connected".to_string()}),
            buffer: [0u8; 1024],
            receiver: Receiver::new(),
            data: Data::None,

            device_type_target: DeviceType::None,
            information_target: Information::new(),
            update_location: UpdateLocation::new(),
            index_target_vec_file: 0,
            index_block_end: 0,
            message_status: String::new(),
            message_version: String::new(),
            update: Update::new(),
            flag_transfer_success: true,
            count_error: 0,

            flag_show_debug_message: false,

            time_transfer: Instant::now(),
            time_receive: Instant::now(),

            time_sequence_start: Instant::now(),
        };

        // 시리얼 포트 목록 생성
        updater.create_port_list();
        
        
        if updater.is_exists_firmware_file() == false
        {
            updater.set_sequence(Sequence::NoFirmwareFile);
        }

        updater
    }


    fn create_port_list(&mut self)
    {
        self.vec_serialport.clear();

        if let Ok(vec_sp_info) = serialport::available_ports()
        {
            for sp_info in vec_sp_info
            {
                self.vec_serialport.push(sp_info.port_name);
            }
        }
    }


    fn is_exists_firmware_file(&self) -> bool
    {
        self.vec_file.len() > 0
    }


    pub fn get_message_status(&self) -> &str
    {
        &self.message_status
    }


    pub fn get_message_version(&self) -> &str
    {
        &self.message_version
    }


    pub fn get_sequence(&self) -> &Sequence
    {
        &self.sequence
    }

    
    fn set_sequence(&mut self, sequence: Sequence)
    {
        match sequence
        {
            Sequence::FirmwareUpdate =>
            {
                self.flag_transfer_success = true;
            }

            Sequence::UpdateComplete =>
            {
                self.message_status = String::from("Update Complete");
            }

            Sequence::NoFirmwareFile =>
            {
                self.message_status = String::from("No Firmware File");
            }

            Sequence::NoMatchedFirmwareFile =>
            {
                self.message_status = String::from("Can't find matched firmware file");
            }

            Sequence::ApplicationMode =>
            {
                self.message_status = String::from("Reconnect with bootloader mode");
            }

            _ => {}
        }

        self.ticker_transfer = Ticker::new(200); // sequence 변경 시 ticker 내부의 카운터를 초기화하기 위함
        self.time_sequence_start = Instant::now();
        self.sequence = sequence;
    }

    
    fn send(&mut self, slice_data: &[u8]) -> bool
    {
        if let Ok(port) = &mut self.port
        {
            if let Ok(_len) = port.write(slice_data)
            {
                self.time_transfer = Instant::now();
                return true;
            }
        }

        false
    }


    fn request(&mut self, target: DeviceType, data_type: DataType) -> bool
    {
        self.send(&transfer::transfer(DataType::Request, DeviceType::Base, target, &Request{data_type}.to_vec()))
    }


    fn check(&mut self) -> &Data
    {
        if let Ok(port) = &mut self.port
        {
            if let Ok(length_read) = &port.read(&mut self.buffer)
            {
                if *length_read > 0
                {
                    if self.flag_show_debug_message 
                    {
                        println!("RX: {:X?}", &self.buffer[..*length_read]);
                    }

                    self.receiver.push_slice(&self.buffer[..*length_read]);
                }
            }

            if let messaging::State::Loaded = self.receiver.check()
            {
                self.receiver.clear();
                self.time_receive = Instant::now();
                self.data = handler::check(self.receiver.get_header(), self.receiver.get_data());
                return &self.data;
            }
        }

        &Data::None
    }


    // script handler
    pub fn run(&mut self)
    {
        match self.sequence
        {
            Sequence::LookUpNewDevice =>
            {
                self.run_look_up_new_device();
            }

            Sequence::CheckDeviceInformation =>
            {
                self.run_check_device_information();
            }

            Sequence::CheckUpdateLocation =>
            {
                self.run_check_update_location();
            }
            
            Sequence::FirmwareUpdate =>
            {
                self.run_firmware_update();
            }
            

            Sequence::NoFirmwareFile =>
            {
                
            }

            _ => {}
        }

    }


    fn lookup_firmware_files() -> Vec<EncryptedBinary>
    {
        let mut vec_file: Vec<EncryptedBinary> = Vec::new();

        // find firmware files
        if let Ok(mut path) = std::env::current_exe()
        {
            path.pop();
            path.push("firmware");

            let paths = fs::read_dir(path);

            if let Ok(p) = paths
            {
                for file in p
                {
                    if let Ok(f) = file
                    {
                        if let Some(fpath) = f.path().to_str()
                        {
                            let mut eb = EncryptedBinary::new();

                            if eb.read(fpath) {
                                /*
                                // 읽은 펌웨어 파일의 정보 표시
                                tx_message_status.send(String::from(
                                    "{} / Name: {}",
                                    vec_file.len(),
                                    f.path().display()
                                );
                                tx_message_status.send(String::from("{:?}", eb.header);
                                */
                                vec_file.push(eb);
                            }
                        }
                    }
                }
            }
        }

        vec_file
    }


    fn lookup_new_device(vec_serialport: &mut Vec<String>) -> Option<String>
    {
        let mut vec_serialport_new: Vec<String> = Vec::new();

        if let Ok(vec_sp_info) = serialport::available_ports()
        {
            for sp_info in vec_sp_info
            {
                vec_serialport_new.push(sp_info.port_name);
            }

            if vec_serialport_new.len() > vec_serialport.len()
            {
                let mut vec_serialport_clone = vec_serialport_new.clone();

                for i in 0..vec_serialport.len()
                {
                    for j in 0..vec_serialport_clone.len()
                    {
                        if vec_serialport[i] == vec_serialport_clone[j]
                        {
                            vec_serialport_clone.remove(j);
                            break;
                        }
                    }
                }

                *vec_serialport = vec_serialport_new;

                if vec_serialport_clone.len() > 0
                {
                    return Some(vec_serialport_clone[0].clone());
                }
            }
            else
            {
                *vec_serialport = vec_serialport_new;
            }
        }

        return None;
    }


    fn run_look_up_new_device(&mut self)
    {
        if let Some(port_name) =  Updater::lookup_new_device(&mut self.vec_serialport)
        {
            self.port = serialport::new(port_name, 57_600)
                .timeout(Duration::from_millis(1))
                .open();
            
            if let Ok(_port) = &mut self.port
            {
                // 시리얼 포트가 정상적으로 열린 경우 장치 정보 확인 모드로 변경
                self.set_sequence(Sequence::CheckDeviceInformation);
            }
            else
            {
                // 연결 할 수 없는 장치 이름을 기존 장치 이름 목록에 넣고 새로운 장치 검색 모드로 복귀
                self.create_port_list();
                self.set_sequence(Sequence::LookUpNewDevice);
            }
        }
    }


    fn run_check_device_information(&mut self)
    {
        // 주기적으로 information 데이터를 요청
        if self.ticker_transfer.check()
        {
            let mut device_type = DeviceType::Drone;

            match self.ticker_transfer.get_count() % 8
            {
                1 => { device_type = DeviceType::Controller; }
                2 => { device_type = DeviceType::LinkClient; }
                3 => { device_type = DeviceType::LinkServer; }
                4 => { device_type = DeviceType::BleClient; }
                5 => { device_type = DeviceType::BleServer; }
                6 => { device_type = DeviceType::Tester; }
                7 => { device_type = DeviceType::Monitor; }
                _ => {}
            }

            self.request(device_type, DataType::Information);
        }

        if let Data::Information(information) = self.check()
        {
            self.information_target = *information;
            self.device_type_target = self.receiver.get_header().from;

            if self.flag_show_debug_message
            {
                println!("Received Information: {:?}", self.information_target);
            }

            if  self.information_target.model_number != ModelNumber::None 
            {
                if self.information_target.mode_update == system::ModeUpdate::Ready || self.information_target.mode_update == system::ModeUpdate::Update
                {
                    // 업데이트를 할 수 있는 장치인 경우 다음 단계로 넘어감
                    if self.find_matched_firmware_file(self.information_target.model_number)
                    {
                        self.set_sequence(Sequence::CheckUpdateLocation);
                    }
                    else
                    {
                        self.set_sequence(Sequence::NoMatchedFirmwareFile);
                    }
                }
                else if self.information_target.mode_update == system::ModeUpdate::Complete
                {
                    self.set_sequence(Sequence::UpdateComplete);
                }
                else if self.information_target.mode_update == system::ModeUpdate::RunApplication
                {
                    self.set_sequence(Sequence::ApplicationMode);
                }
            }
        }
        
        // 원하는 데이터를 얻지 못하고 시간을 초과하는 경우 새로운 장치 검색 모드로 변경
        if self.time_sequence_start.elapsed().as_millis() > 1200
        {
            self.create_port_list();
            self.set_sequence(Sequence::LookUpNewDevice);
        }
    }


    fn find_matched_firmware_file(&mut self, model_number: ModelNumber) -> bool
    {
        for i in 0..self.vec_file.len()
        {
            // 펌웨어 파일의 모델 번호와 연결된 장치의 모델 번호가 일치하는지 확인
            if self.vec_file[i].header.model_number == model_number
            {
                self.index_target_vec_file = i;
                self.index_block_end = (self.vec_file[i].data_array.len() >> 4) as u16;

                self.message_version = format!(
                    "{}.{}.{} -> {}.{}.{}",
                    self.information_target.version.major,
                    self.information_target.version.minor,
                    self.information_target.version.build,
                    self.vec_file[i].header.version.major,
                    self.vec_file[i].header.version.minor,
                    self.vec_file[i].header.version.build);

                return true;
            }
        }

        false
    }


    fn run_check_update_location(&mut self)
    {
        // 주기적으로 information 데이터를 요청
        if self.ticker_transfer.check()
        {
            self.request(self.device_type_target, DataType::UpdateLocation);
        }

        if let Data::UpdateLocation(update_location) = &mut self.check()
        {
            self.update_location = update_location.clone();

            if self.flag_show_debug_message
            {
                println!("Received UpdateLocation: {:?}", self.update_location);
            }

            self.set_sequence(Sequence::FirmwareUpdate);
            return;
        }
        
        // 원하는 데이터를 얻지 못하고 시간을 초과하는 경우 새로운 장치 검색 모드로 변경
        if self.time_sequence_start.elapsed().as_millis() > 1200
        {
            self.create_port_list();
            self.set_sequence(Sequence::LookUpNewDevice);
        }
    }


    fn run_firmware_update(&mut self)
    {
        if self.flag_transfer_success || self.ticker_transfer.check() 
        {
            if self.flag_transfer_success == false
            {
                self.count_error += 1;
            }

            self.flag_transfer_success = false;

            if let Some(vec_data) = self.vec_file[self.index_target_vec_file].get_data_block(self.update_location.index_block_next, 2)
            {
                self.update.index_block_next = self.update_location.index_block_next;
                self.update.vec_data = vec_data;

                self.send(&transfer::transfer(
                    DataType::Update,
                    DeviceType::Base,
                    self.device_type_target,
                    &self.update.to_vec(),
                ));
            }
        }
        
        match self.check().clone()
        {
            Data::UpdateLocation(update_location_new) =>
            {
                if self.update.index_block_next != update_location_new.index_block_next
                {
                    self.count_error = 0;
                    self.flag_transfer_success = true;
                    self.update_location = update_location_new.clone();
                }
            }

            Data::Information(information) =>
            {
                if information.mode_update == system::ModeUpdate::Complete
                {
                    self.set_sequence(Sequence::UpdateComplete);
                }
            }

            _ => {}
        }

        // 에러가 일정 이상 쌓이면 오류 처리하고 업데이트 중단
        if self.count_error > 30
        {
            self.set_sequence(Sequence::NoAnswer);
        }

        // 업데이트 제한 시간 초과
        if self.time_sequence_start.elapsed().as_millis() > 300000
        {
            self.set_sequence(Sequence::UpdateTimeOver);
        }
    }


    pub fn get_update_information(&self) -> (i32, i32, i32, f32)
    {
        if let Sequence::FirmwareUpdate = self.sequence
        {
            let time_progress = self.time_sequence_start.elapsed().as_millis() as i32;
            let time_total = time_progress * self.index_block_end as i32 / self.update.index_block_next as i32;
            let time_left = time_total - time_progress;

            let progress: f32 = self.update_location.index_block_next as f32 * 100.0 / self.index_block_end as f32;

            (time_total, time_progress, time_left, progress)
        }
        else
        {
            (0, 0, 0, 0_f32)
        }
    }
}
