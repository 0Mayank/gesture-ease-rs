use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gesture_ease::config::Config;
use gesture_ease::math::{
    angle_bw_cameras_from_z_axis, calc_position, get_closest_device_in_los, get_los, sort_align,
};
use gesture_ease::models::{GesturePreds, HPEPreds, HeadPreds};
use gesture_ease::{GError, HasGlamQuat, HasImagePosition, Models};
use libcamera::camera_manager::CameraManager;
use libcamera::framebuffer_allocator::{FrameBuffer, FrameBufferAllocator};
use libcamera::framebuffer_map::MemoryMappedFrameBuffer;
use libcamera::stream::StreamRole;

fn main() {
    let socket_path = "/tmp/gesurease.sock";
    let num_processes = 3;

    if std::fs::metadata(socket_path).is_ok() {
        println!("Socket is already present. Deleting...");
        std::fs::remove_file(socket_path).unwrap();
    }

    let config = Config::open("config.toml".into()).unwrap();

    let listener = UnixListener::bind(socket_path).unwrap();
    let mut process_map = Models::new(num_processes, listener);

    let mgr = CameraManager::new().unwrap();
    let cameras = mgr.cameras();

    let cam1 = cameras.get(0).expect("Camera 0 not found");
    let mut cam1 = cam1.acquire().expect("Unable to aquire camera 0");
    let cam2 = cameras.get(1).expect("Camera 1 not found");
    let mut cam2 = cam2.acquire().expect("Unable to aquire camera 1");

    let mut cfgs1 = cam1
        .generate_configuration(&[StreamRole::StillCapture])
        .unwrap();
    let mut cfgs2 = cam2
        .generate_configuration(&[StreamRole::StillCapture])
        .unwrap();

    cfgs1
        .get_mut(0)
        .unwrap()
        .set_pixel_format(config.camera1.format.pixel_format());
    cfgs2
        .get_mut(0)
        .unwrap()
        .set_pixel_format(config.camera2.format.pixel_format());

    dbg!(&cfgs1);
    dbg!(&cfgs2);

    cam1.configure(&mut cfgs1)
        .expect("Unable to configure camera");
    cam2.configure(&mut cfgs2)
        .expect("Unable to configure camera");

    let stream1 = cfgs1.get(0).unwrap().stream().unwrap();
    let stream2 = cfgs2.get(1).unwrap().stream().unwrap();

    let (tx1, rx1) = std::sync::mpsc::channel();
    cam1.on_request_completed(move |req| {
        tx1.send(req).unwrap();
    });
    let (tx2, rx2) = std::sync::mpsc::channel();
    cam2.on_request_completed(move |req| {
        tx2.send(req).unwrap();
    });

    cam1.start(None).unwrap();
    cam2.start(None).unwrap();

    let theta = angle_bw_cameras_from_z_axis(&config.camera1, &config.camera2);

    let mut headposes: HPEPreds = Default::default();
    let mut gestures: GesturePreds = Default::default();
    let mut head_positions: HeadPreds = Default::default();

    let mut run = || -> error_stack::Result<(), GError> {
        process_map.wait_for_connection();

        let mut alloc1 = FrameBufferAllocator::new(&cam1);
        let mut alloc2 = FrameBufferAllocator::new(&cam2);

        let buffer1 = alloc1
            .alloc(&stream1)
            .unwrap()
            .into_iter()
            .map(|buf| MemoryMappedFrameBuffer::new(buf).unwrap())
            .last()
            .unwrap();
        let buffer2 = alloc2
            .alloc(&stream2)
            .unwrap()
            .into_iter()
            .map(|buf| MemoryMappedFrameBuffer::new(buf).unwrap())
            .last()
            .unwrap();

        let mut req1 = cam1.create_request(None).unwrap();
        let mut req2 = cam2.create_request(None).unwrap();

        req1.add_buffer(&stream1, buffer1).unwrap();
        req2.add_buffer(&stream2, buffer2).unwrap();

        cam1.queue_request(req1).unwrap();
        cam2.queue_request(req2).unwrap();

        let rec1 = rx1
            .recv_timeout(Duration::from_secs(2))
            .expect("Camera 0 request failed");
        let rec2 = rx2
            .recv_timeout(Duration::from_secs(2))
            .expect("Camera 1 request failed");

        let frame1: &MemoryMappedFrameBuffer<FrameBuffer> = rec1.buffer(&stream1).unwrap();
        let frame2: &MemoryMappedFrameBuffer<FrameBuffer> = rec2.buffer(&stream2).unwrap();

        let frame1: Arc<[u8]> = frame1.data().concat().into();
        let frame2: Arc<[u8]> = frame2.data().concat().into();

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
        sort_align(&mut head_positions, theta);
        gestures = process_map.gesture()?.recv()?;
        sort_align(&mut gestures, theta);

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
        process_map.hpe()?.send(
            frame1.clone(),
            config.camera1.img_width,
            config.camera1.img_height,
        )?;

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

        Ok(())
    };

    loop {
        let start = Instant::now();
        run().unwrap();
        let duration = Instant::now().duration_since(start).as_millis();
        println!("duration in ms: {}", duration);
    }
}
