use gesture_ease::config::Config;
use gesture_ease::head_detection::HeadPreds;
use gesture_ease::math::{calc_position, get_closest_device_in_los, get_los};
use gesture_ease::{GError, GesturePreds, GlamQuat, HPEPreds, ImagePosition, Models};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
};
use nokhwa::Camera;
use std::io::Cursor;
use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let socket_path = "/tmp/gesurease.sock";
    let num_processes = 2;

    if std::fs::metadata(socket_path).is_ok() {
        println!("Socket is already present. Deleting...");
        std::fs::remove_file(socket_path).unwrap();
    }

    let config = Config::open("config.toml".into()).unwrap();

    let listener = UnixListener::bind(socket_path).unwrap();
    let mut process_map = Models::new(num_processes, listener);

    let index1 = CameraIndex::Index(0);
    let index2 = CameraIndex::Index(1);
    let height = 720;
    let width = 1280;
    let resolution = Resolution::new(width, height);
    let frame_format = FrameFormat::MJPEG;
    let camera_format = CameraFormat::new(resolution, frame_format, 30);
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(camera_format));

    let mut camera1 = Camera::new(index1, requested).unwrap();
    let mut camera2 = Camera::new(index2, requested).unwrap();
    camera1.open_stream().unwrap();
    camera2.open_stream().unwrap();

    let mut img1 = Cursor::new(vec![]);
    let mut img2 = Cursor::new(vec![]);

    let mut headposes: HPEPreds = Default::default();
    let mut gestures: GesturePreds = Default::default();
    let mut head_positions: HeadPreds = Default::default();

    let mut run = || -> error_stack::Result<(), GError> {
        process_map.wait_for_connection();

        let frame1 = camera1.frame().unwrap();
        let frame2 = camera2.frame().unwrap();

        frame1
            .decode_image::<RgbFormat>()
            .unwrap()
            .write_to(&mut img1, image::ImageFormat::Jpeg)
            .unwrap();
        frame2
            .decode_image::<RgbFormat>()
            .unwrap()
            .write_to(&mut img2, image::ImageFormat::Jpeg)
            .unwrap();

        let buffer1: Arc<[u8]> = img1.get_ref().to_owned().into();
        let buffer2: Arc<[u8]> = img1.get_ref().to_owned().into();

        // send frame1 to gesture detection model
        process_map.gesture()?.send(buffer1.clone())?;
        // send frame2 to head detection model
        process_map.head_detection()?.send(buffer2.clone())?;

        head_positions = process_map.head_detection()?.recv()?;
        head_positions.sort_by(|a, b| a.nose_x.partial_cmp(&b.nose_x).expect("NANANANANA"));
        gestures = process_map.gesture()?.recv()?;
        gestures.sort_by(|a, b| a.nose_x.partial_cmp(&b.nose_x).expect("NANI!?"));

        // check if any gesture is ok
        if gestures
            .iter()
            .map(|x| &x.gesture)
            .find(|x| x.is_toggle())
            .is_some()
        {
            return Ok(());
        }

        // send frame1 to hpe model
        process_map.hpe()?.send(buffer1.clone())?;

        // in the meantime calculate positition of head which had a gesture
        let positions = gestures.iter().zip(head_positions.iter()).map(|(g, h)| {
            if !g.is_none() {
                Some((
                    calc_position(
                        &config.camera1,
                        &g.image_coords(width, height),
                        &config.camera2,
                        &h.image_coords(width, height),
                    )
                    .unwrap(),
                    g.gesture.clone(),
                ))
            } else {
                None
            }
        });

        headposes = process_map.hpe().unwrap().recv().unwrap();
        headposes.sort_by(|a, b| a.x1.partial_cmp(&b.x1).expect("NAN??"));

        // Now get the device in line of sight of each head
        let devices = headposes.iter().zip(positions).map(|(pose, position)| {
            let (position, gesture) = if let Some((position, gesture)) = position {
                (position, gesture)
            } else {
                return None;
            };

            let line_of_sight = get_los(&config.camera1, &position, &pose.quat());

            get_closest_device_in_los(&config, line_of_sight).map(|x| (x, gesture))
        });

        devices.for_each(|x| {
            if let Some((device, gesture)) = x {
                println!("gesture {:?} on device {}", gesture, device.name);
            }
        });

        img1.set_position(0);
        img2.set_position(0);

        Ok(())
    };

    loop {
        let start = Instant::now();
        run().unwrap();
        let duration = Instant::now().duration_since(start).as_millis();
        println!("duration in ms: {}", duration);
    }
}
