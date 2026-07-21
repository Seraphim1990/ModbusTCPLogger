use std::{fs, io};
use std::process::Command;
use std::time::Duration;

// Секретні ключі, для рівня "від чесних людей" цього вистачить
const XOR_KEY_CHALLENGE: u8 = 0xAA; // Ключ для першої тарабарщини (запит)
const XOR_KEY_LICENSE: u8 = 0x55;   // Ключ для другої тарабарщини (ліцензія)

// Імітація отримання ID диска (твоя функція з минулого кроку)

fn get_disk_serial() -> String {
    // Для тесту, якщо powershell видасть помилку, повернемо заглушку
    let output = Command::new("powershell")
        .args(&["-Command", "Get-WmiObject -Class Win32_PhysicalMedia | Select-Object -First 1 -ExpandProperty SerialNumber"])
        .output();

    if let Ok(out) = output {
        let serial = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !serial.is_empty() { return serial; }
    }
    "DISK_SERIAL_FALLBACK_12345".to_string()
}

// Перетворення байтів у читаєму "тарабарщину" (Hex)
fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// Зворотне перетворення з Hex у байти
fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 { return None; }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

// Простий XOR для шифрування/дешифрування
fn xor_cipher(data: &[u8], key: u8) -> Vec<u8> {
    data.iter().map(|&b| b ^ key).collect()
}

pub fn check_key_and_activate() {
    let license_dir = "configs";
    let license_file = "configs/license.key";

    // Переконуємось, що папка config існує
    let _ = fs::create_dir_all(license_dir);

    loop {
        let current_id = get_disk_serial();

        // 1. Перевірка наявного файлу ліцензії
        if let Ok(license_hex) = fs::read_to_string(license_file) {
            if let Some(license_bytes) = hex_to_bytes(license_hex.trim()) {
                let decrypted_bytes = xor_cipher(&license_bytes, XOR_KEY_LICENSE);
                if let Ok(decrypted_id) = String::from_utf8(decrypted_bytes) {
                    if decrypted_id == current_id {
                        return; // Виходимо з функції, пускаємо далі в мейн
                    }
                }
            }
            println!("[Помилка] Файл ліцензії не підходить для цього ПК або пошкоджений.");
        }

        // 2. Якщо ліцензії немає або вона не підійшла — генеруємо код запиту
        let challenge_bytes = xor_cipher(current_id.as_bytes(), XOR_KEY_CHALLENGE);
        let challenge_hex = bytes_to_hex(&challenge_bytes);

        println!("\n==================================================");
        println!("ПРОГРАМА НЕ АКТИВОВАНА!");
        println!("Передайте цей код розробнику:");
        println!("{}", challenge_hex);
        println!("==================================================");
        println!("\nВведіть ліцензійний ключ, отриманий від розробника:");

        // 3. Читаємо ключ з консолі замість заглушки-сну
        let mut user_input = String::new();
        if io::stdin().read_line(&mut user_input).is_ok() {
            let clean_input = user_input.trim();

            // Валідуємо те, що ввів користувач прямо зараз
            if let Some(input_bytes) = hex_to_bytes(clean_input) {
                let decrypted_bytes = xor_cipher(&input_bytes, XOR_KEY_LICENSE);
                if let Ok(decrypted_id) = String::from_utf8(decrypted_bytes) {
                    if decrypted_id == current_id {
                        // Якщо введений ключ правильний — зберігаємо його у файл!
                        if fs::write(license_file, clean_input).is_ok() {
                            println!("[Успіх] Ключ підійшов! Ліцензію збережено.");
                            std::thread::sleep(Duration::from_secs(5));
                            print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
                            return; // Все супер, ліцензовано
                        } else {
                            println!("[Помилка] Не вдалося записати файл ліцензії. Перевірте права доступу.");
                        }
                    }
                }
            }
            println!("\n[Помилка] Невірний ліцензійний ключ! Спробуйте ще раз.");
        }
    }
}