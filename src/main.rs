use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::time::Instant;

use gesture_ease::config::Config;
use gesture_ease::math::{
    angle_bw_cameras_from_z_axis, calc_position, get_closest_device_in_los, get_los, sort_align,
};
use gesture_ease::models::{GesturePreds, HPEPreds, HeadPreds};
use gesture_ease::{GError, HasGlamQuat, HasImagePosition, Models};

fn main() {
    let socket_path = "/tmp/gesurease.sock";
    let num_processes = 4;

    if std::fs::metadata(socket_path).is_ok() {
        println!("Socket is already present. Deleting...");
        std::fs::remove_file(socket_path).unwrap();
    }

    let config = Config::open("config.toml".into()).unwrap();

    let listener = UnixListener::bind(socket_path).unwrap();
    let mut process_map = Models::new(num_processes, listener);

    let theta = angle_bw_cameras_from_z_axis(&config.camera1, &config.camera2);

    let mut headposes: HPEPreds = Default::default();
    let mut gestures: GesturePreds = Default::default();
    let mut head_positions: HeadPreds = Default::default();

    process_map.wait_for_connection(&config);

    let mut run = || -> error_stack::Result<(), GError> {
        let frames = process_map.cams()?.recv()?;

        let frame1: Arc<[u8]> = frames.cam1.into();
        let frame2: Arc<[u8]> = frames.cam2.into();

        // send frame1 to gesture detection model
        process_map.gesture()?.send(
            frame1.clone(),
            config.camera1.img_width,
            config.camera1.img_height,
        )?;
        // send frame2 to head detection model
        process_map.head_detection()?.send(
            frame2.clone(),
            config.camera2.img_width,
            config.camera2.img_height,
        )?;

        head_positions = process_map.head_detection()?.recv()?;
        gestures = process_map.gesture()?.recv()?;

        // check if any gesture is not none
        if gestures
            .iter()
            .map(|x| &x.gesture)
            .find(|x| x.is_toggle())
            .is_some()
        {
            // send frame1 to hpe model
            process_map.hpe()?.send(
                frame1.clone(),
                config.camera1.img_width,
                config.camera1.img_height,
            )?;

            sort_align(&mut head_positions, theta);
            sort_align(&mut gestures, theta);
            // in the meantime calculate positition of head which had a gesture
            let positions = gestures.iter().zip(head_positions.iter()).map(|(g, h)| {
                if !g.is_none() {
                    Some((
                        calc_position(
                            &config.camera1,
                            &g.image_coords(config.camera1.img_width, config.camera1.img_height),
                            &config.camera2,
                            &h.image_coords(config.camera2.img_width, config.camera2.img_height),
                        )
                        .unwrap(),
                        g.gesture.clone(),
                    ))
                } else {
                    None
                }
            });

            headposes = process_map.hpe().unwrap().recv().unwrap();
            sort_align(&mut headposes, theta);

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
        }

        Ok(())
    };

    loop {
        let start = Instant::now();
        run().unwrap();
        let duration = Instant::now().duration_since(start).as_millis();
        println!("duration in ms: {}", duration);
    }
}
