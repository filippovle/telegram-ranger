use super::*;
use ::image::{ImageBuffer, Rgba, RgbaImage};
use ::imageproc::drawing::draw_text_mut;
use rand::{rng, Rng};
use rusttype::{Font, Scale};
use teloxide::types::InputFile;

// --- Наборы символов ---

// Без неоднозначных латинских (нет I,l,1,O,0)
const LATIN_SAFE: &[char] = &[
    'A', 'B', 'C', 'D', 'F', 'G', 'H', 'J', 'K', 'M', 'N', 'P', 'Q', 'R', 'T', 'V', 'W', 'X', 'Y',
    'Z',
];
// Без кириллиц, похожих на латиницу (А,Е,К,М,Н,О,Р,С,Т,У,Х)
const CYRILLIC_SAFE: &[char] = &[
    'Б', 'Г', 'Д', 'Ж', 'З', 'И', 'Й', 'Л', 'П', 'Ф', 'Ц', 'Ч', 'Ш', 'Щ', 'Э', 'Ю', 'Я',
];
// Цифры без 0/1
const DIGITS_SAFE: &[char] = &['2', '3', '4', '5', '6', '7', '8', '9'];

pub struct ImageCaptcha;

#[async_trait]
impl Captcha for ImageCaptcha {
    fn mode(&self) -> CaptchaMode {
        CaptchaMode::Image
    }

    async fn ask(
        &self,
        bot: &Bot,
        state: &AppState,
        chat_id: ChatId,
        user: &User,
    ) -> Result<Challenge> {
        let alphabet = select_alphabet();
        let code = gen_code_from(&alphabet, 5);

        let caption = format!(
            "{}, введите символы с картинки за {} секунд (без пробелов, без регистра).",
            mention(bot, chat_id, user.id).await,
            state.cfg.captcha_timeout_secs
        );

        let png_bytes = render_captcha_png(&code)?;
        let msg = bot
            .send_photo(
                chat_id,
                InputFile::memory(png_bytes).file_name("captcha.png"),
            )
            .caption(caption)
            .parse_mode(ParseMode::Html)
            .await?;

        Ok(Challenge {
            message: msg,
            expected_answer: Some(code.to_lowercase()),
        })
    }

    async fn on_text(&self, _bot: &Bot, state: Arc<AppState>, msg: &Message) -> Result<bool> {
        let Some(from) = msg.from() else {
            return Ok(false);
        };
        let chat_id = msg.chat.id;
        let key = AppState::key(chat_id, from.id);
        let Some((_k, pend)) = state.pending.get(&key).map(|r| (key, r.clone())) else {
            return Ok(false);
        };

        if let (Some(ans), Some(txt)) = (pend.expected_answer.as_ref(), msg.text()) {
            Ok(normalize(txt) == *ans)
        } else {
            Ok(false)
        }
    }
}

// --- вспомогательное ---

fn select_alphabet() -> Vec<char> {
    // CAPTCHA_ALPHABET=latin|cyrillic|both (по умолчанию cyrillic)
    match std::env::var("CAPTCHA_ALPHABET").as_deref() {
        Ok("latin")    => [LATIN_SAFE, DIGITS_SAFE].concat(),
        Ok("both")     => [LATIN_SAFE, CYRILLIC_SAFE, DIGITS_SAFE].concat(),
        _ /* cyrillic*/=> [CYRILLIC_SAFE, DIGITS_SAFE].concat(),
    }
}

fn gen_code_from(alphabet: &[char], len: usize) -> String {
    let mut r = rng();
    (0..len)
        .map(|_| {
            let idx = r.random_range(0..alphabet.len());
            alphabet[idx]
        })
        .collect()
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Рисуем CAPTCHA PNG: увеличенные поля, центрирование текста, больше шума.
fn render_captcha_png(code: &str) -> Result<Vec<u8>> {
    // Чуть больше холст, чтобы превью Telegram не «съедало» символы по краям
    const W: u32 = 360;
    const H: u32 = 140;
    const PAD: i32 = 18; // безопасный отступ от краёв
    const TEXT_AREA_H_FRAC: f32 = 0.65;

    let mut img: RgbaImage = ImageBuffer::from_pixel(W, H, Rgba([255, 255, 255, 255]));

    // Слой шума ДО текста
    add_noise(&mut img);

    // Шрифт (кириллица поддерживается)
    let font_data: &[u8] = include_bytes!("../../assets/DejaVuSans-Bold.ttf");
    let font = Font::try_from_bytes(font_data).ok_or_else(|| anyhow::anyhow!("Invalid font"))?;

    // Масштаб шрифта адаптивный: под высоту и длину кода
    let cell_w = ((W as i32 - PAD * 2) as f32) / (code.chars().count().max(1) as f32);
    let target_h = (H as f32 * TEXT_AREA_H_FRAC).min(64.0);
    let scale = Scale {
        x: target_h,
        y: target_h,
    };

    // Вертикальная базовая позиция по центру
    let base_y = (H as i32 / 2) - (target_h as i32 / 2);

    // Рисуем каждый символ в центре своей «ячейки», с лёгким джиттером
    for (i, ch) in code.chars().enumerate() {
        // горизонтальная «ячейка»
        let cx = PAD as f32 + (i as f32 + 0.5) * cell_w;

        // небольшой джиттер/наклон по позиции
        let jx = rand::rng().random_range(-(cell_w as i32 / 6)..=(cell_w as i32 / 6));
        let jy = rand::rng().random_range(-10..=10);

        // Тень (слегка смещённая) для читаемости
        draw_text_mut(
            &mut img,
            Rgba([0, 0, 0, 60]),
            (cx as i32) - (scale.x as i32 / 3) + jx + 1,
            base_y + jy + 1,
            scale,
            &font,
            &ch.to_string(),
        );

        // Сам символ
        draw_text_mut(
            &mut img,
            Rgba([0, 0, 0, 255]),
            (cx as i32) - (scale.x as i32 / 3) + jx,
            base_y + jy,
            scale,
            &font,
            &ch.to_string(),
        );
    }

    // Дополнительные помехи ПОСЛЕ текста (полупрозрачные линии поверх)
    add_overlay_noise(&mut img);

    // PNG в память
    let mut buf = Vec::new();
    {
        let mut encoder = ::image::codecs::png::PngEncoder::new(&mut buf);
        use ::image::ColorType;
        encoder.encode(&img, W, H, ColorType::Rgba8)?;
    }
    Ok(buf)
}

fn add_noise(img: &mut RgbaImage) {
    let mut r = rng();

    // Случайные линии + псевдо-«толстые» (рисуем параллельно)
    for _ in 0..14 {
        let (x1, y1) = (
            r.random_range(0..img.width() as i32),
            r.random_range(0..img.height() as i32),
        );
        let (x2, y2) = (
            r.random_range(0..img.width() as i32),
            r.random_range(0..img.height() as i32),
        );
        let color = Rgba([r.random(), r.random(), r.random(), 180]);
        let w = r.random_range(1..=3);
        draw_thick_line(img, x1, y1, x2, y2, w, color);
    }

    // Точки
    for _ in 0..(img.width() * 2) {
        let (x, y) = (
            r.random_range(0..img.width()),
            r.random_range(0..img.height()),
        );
        img.put_pixel(x, y, Rgba([r.random(), r.random(), r.random(), 200]));
    }
}

fn add_overlay_noise(img: &mut RgbaImage) {
    let mut r = rng();
    for _ in 0..10 {
        let (x1, y1) = (
            r.random_range(0..img.width() as i32),
            r.random_range(0..img.height() as i32),
        );
        let (x2, y2) = (
            r.random_range(0..img.width() as i32),
            r.random_range(0..img.height() as i32),
        );
        let color = Rgba([r.random(), r.random(), r.random(), 120]); // полупрозрачные поверх текста
        let w = r.random_range(1..=2);
        draw_thick_line(img, x1, y1, x2, y2, w, color);
    }
}

// «Толстая» линия — набор параллельных 1px
fn draw_thick_line(
    img: &mut RgbaImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    width: i32,
    color: Rgba<u8>,
) {
    for off in -(width / 2)..=(width / 2) {
        draw_line(img, x0 + off, y0, x1 + off, y1, color);
        draw_line(img, x0, y0 + off, x1, y1 + off, color);
    }
}

// Брезенхэм 1px
fn draw_line(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (x0, y0);
    loop {
        if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
            img.put_pixel(x as u32, y as u32, color);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_charset_and_len() {
        let a = [LATIN_SAFE, CYRILLIC_SAFE, DIGITS_SAFE].concat();
        let s = gen_code_from(&a, 6);
        assert_eq!(s.chars().count(), 6);
        assert!(s.chars().all(|ch| a.contains(&ch)));
    }

    #[test]
    fn normalize_basic() {
        assert_eq!(normalize(" a Б  3 "), "аб3");
        assert_eq!(normalize("A B C"), "abc");
    }

    #[test]
    fn png_nonempty() {
        let png = render_captcha_png("AB3ДЯ").unwrap();
        assert!(png.len() > 1024);
    }
}
