use std::thread;
use std::time::Duration;

fn main() {
    // Seconds thread
    let seconds_thread = thread::spawn(move || {
        let mut s = 0;
        loop {
            println!("Seconds: {}", s);
            s += 1;
            if s == 60 {
                s = 0;
            }
            thread::sleep(Duration::from_secs(1));
        }
    });

    // Minutes thread
    let minutes_thread = thread::spawn(move || {
        let mut m = 0;
        let mut counter = 0;
        loop {
            println!("Minutes: {}", m);
            counter += 1;
            if counter == 60 {
                m += 1;
                counter = 0;
            }
            thread::sleep(Duration::from_secs(1));
        }
    });

    // Join the threads
    seconds_thread.join().unwrap();
    minutes_thread.join().unwrap();
}
