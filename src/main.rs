use std::{io::{stdout, Write, Stdout}, time::Duration, env, fs, cmp::min};
use crossterm::{cursor, event::{self, Event, KeyEvent, KeyCode, KeyModifiers, KeyEventKind}, execute, queue, style, terminal::{self, ClearType}, Result};

macro_rules! prompt {
    ($editor:expr,$message:expr,$default:expr $(, $callback:expr)?) => {{
        let editor: &mut Editor = $editor;
        let message: &str = $message;
        let mut input: String = $default;
        loop {
            editor.set_status_message(Some(format!("{} {}", message, input)));
            editor.refresh_screen()?;
            let event = read_key()?;
            match event {
                KeyEvent{code: KeyCode::Esc, ..} => {
                    input.clear();
                    break;
                },
                KeyEvent{code: KeyCode::Enter, ..} => break,
                KeyEvent{code: KeyCode::Char(c), kind: KeyEventKind::Press, ..} => {
                    input.push(c);
                }
                KeyEvent{code: KeyCode::Backspace, kind: KeyEventKind::Press, ..} => {
                    if input.len() > 0 {input.remove(input.len() - 1);}
                }
                _ => {}
            }
            if let KeyEvent{code: c, kind: KeyEventKind::Press, ..} = event {
                $($callback(editor, &input, c);)?   
            }
        }
        if input.len() > 0 {
            Some(input)
        }else {
            None
        }
    }};
}

fn read_key() -> std::io::Result<KeyEvent> {
    loop {
        while event::poll(Duration::from_millis(500))? {
            if let Event::Key(event) = event::read()? {
                return Ok(event);
            }
        }
    }
}

struct TextField {
    cursor_x: u16,
    cursor_y: u16,
    render_x: u16,
    x_offset: u16,
    y_offset: u16,
    size: (u16, u16),
    lines: Vec<String>,
    dirty: bool,
}

impl TextField {
    fn new(size: (u16, u16)) -> Self {
        Self{cursor_x: 0, cursor_y: 0, render_x: 0, x_offset: 0, y_offset: 0, size: size, lines: vec![String::new()], dirty: true}
    }

    fn load(&mut self, file_name: &String) {
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.render_x = 0;
        self.x_offset = 0;
        self.y_offset = 0;
        self.dirty = false;
        let file_contents = fs::read_to_string(&file_name);
        self.lines = match file_contents {
            Ok(contents) => {
                let mut lines: Vec<String> = contents.lines().map(|it| it.into()).collect();
                if lines.len() == 0 {lines.push(String::new());}
                lines
            },
            _ => vec![String::new()]
        }
    }

    fn save(&mut self, file_name: &String) -> std::io::Result<()>{
        let mut file = fs::OpenOptions::new().write(true).create(true).open(file_name)?;
        let contents = self.lines.join("\n");
        file.set_len(contents.len() as u64)?;
        file.write_all(contents.as_bytes())?;
        self.dirty = false;
        Ok(())
    }

    fn print_line(&self, w: &mut Stdout, y: usize) -> std::io::Result<()> {
        let line_index = y + self.y_offset as usize;
        if line_index < self.lines.len() {
            let full_line = &self.lines[line_index];
            let start = min(self.x_offset as usize, full_line.len());
            let end = min((self.x_offset + self.size.0) as usize , full_line.len());
            let segment = &full_line[start..end];
            queue!(w, cursor::MoveTo(2, 2 + y as u16), style::Print(segment))?;       
        }
        Ok(())
    }

    fn get_cursor_position(&self) -> (u16, u16) {
        (self.render_x + 2 - self.x_offset, self.cursor_y + 2 - self.y_offset)
    }
    
    fn current_line_len(&self) -> u16{
        self.lines[self.cursor_y as usize].len() as u16
    }

    fn change_offset(&mut self) {
        if self.cursor_y < self.y_offset{   // Up
            self.y_offset -= self.y_offset - self.cursor_y;
        }
        if self.render_x > self.size.0 + self.x_offset - 1{     // Right
            self.x_offset += self.render_x - (self.size.0 + self.x_offset - 1);
        }
        if self.cursor_y > self.size.1 + self.y_offset - 1{ // Down
            self.y_offset += self.cursor_y - (self.size.1 + self.y_offset - 1);
        }
        if self.render_x < self.x_offset{   // Left
            self.x_offset -= self.x_offset - self.render_x;
        }
    }

    fn move_cursor(&mut self, direction: KeyCode) {
        match direction {
            KeyCode::Up => {
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                    self.render_x = min(self.cursor_x, self.current_line_len())
                }
            }
            KeyCode::Right => {
                self.cursor_x = self.render_x;
                if self.cursor_x < self.current_line_len() {
                    self.cursor_x += 1;
                }else if self.cursor_y < self.lines.len() as u16 - 1 {
                    self.cursor_y += 1;
                    self.cursor_x = 0;
                }
                self.render_x = self.cursor_x;
            }
            KeyCode::Down => {
                if self.cursor_y < self.lines.len() as u16 - 1 {
                    self.cursor_y += 1;
                    self.render_x = min(self.cursor_x, self.current_line_len())
                }
            }
            KeyCode::Left => {
                self.cursor_x = self.render_x;
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                }else if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                    self.cursor_x = self.current_line_len();
                }
                self.render_x = self.cursor_x;
            }
            _ => {}
        }
        self.change_offset();
    }

    fn find_phrase(&mut self, phrase: &String, key_code: KeyCode) {
        match key_code {
            KeyCode::Char(_) | KeyCode::Backspace => {
                for row in 0..self.lines.len() {
                    if let Some(index) = self.lines[row].find(phrase) {
                        self.cursor_y = row as u16;
                        self.cursor_x = index as u16;
                        self.render_x = index as u16;
                        self.change_offset();
                        break;
                    }
                }
            },
            _ => {}
        }
    }

    fn insert_char(&mut self, c: char) {
        match c{
            '\t' => {
                self.lines[self.cursor_y as usize].insert_str(self.render_x as usize, "    ");
                self.cursor_x = self.render_x + 4;
            }
            _ => {
                self.lines[self.cursor_y as usize].insert(self.render_x as usize, c);
                self.cursor_x = self.render_x + 1;
            }
        }
        self.render_x = self.cursor_x;
        self.dirty = true;
    }

    fn new_line(&mut self) {
        let line_index = self.cursor_y as usize;
        let (old, new) = self.lines[line_index].split_at(self.render_x as usize).clone();
        let (old, new) = (String::from(old), String::from(new));
        self.lines[line_index] = String::from(old);
        self.lines.insert(line_index + 1, String::from(new));
        self.cursor_y += 1;
        self.cursor_x = 0;
        self.render_x = 0;
        self.dirty = true;
    }

    fn delete_char(&mut self) {
        let line_index = self.cursor_y as usize;
        if self.render_x > 0 {
            self.lines[line_index].remove(self.render_x as usize - 1);
            self.cursor_x = self.render_x - 1;
        }else if self.cursor_y > 0 {
            let current_line = self.lines[self.cursor_y as usize].clone();
            self.cursor_x = self.lines[line_index - 1].len() as u16;
            self.lines[line_index - 1].push_str(&current_line.as_str());
            self.lines.remove(line_index);
            self.cursor_y -= 1;
        }
        self.render_x = self.cursor_x;
        self.dirty = true;
    }

    fn is_dirty(&self) -> bool {
        self.dirty
    }
}

struct Editor {
    running: bool,
    win_size: (u16, u16),
    w: Stdout,
    file_name: Option<String>,
    text_field: TextField,
    status_message: Option<String>
}

impl Editor{
    fn new() -> Self {
        crossterm::terminal::enable_raw_mode().unwrap();
        let win_size = terminal::size().unwrap();
        let file_name = env::args().nth(1);
        let mut text_field = TextField::new((win_size.0 - 2, win_size.1 - 3));
        if let Some(name) = &file_name {
            text_field.load(name);
        }
        Self { running: true, win_size, w: stdout(), file_name: file_name, text_field: text_field, status_message: None}
    }

    fn print_header(&mut self) -> std::io::Result<()> {
        let ver = option_env!("CARGO_PKG_VERSION").expect("Could not find version");
        let file_name = match &self.file_name {
            Some(name) => format!("{}{}", if self.text_field.is_dirty() {"*"} else {""}, name),
            None => String::from("Untitled")
        };
        let mut welcome_message = format!("{} -- Christopher's text editor -- {}", file_name, ver);
        welcome_message.truncate(self.win_size.0 as usize);
        queue!(&mut self.w, cursor::MoveTo(0,0), style::Print(welcome_message))
    }

    fn set_status_message(&mut self, message: Option<String>) {
        self.status_message = message;
    }

    fn get_status_message(&self) -> String {
        match &self.status_message {
            Some(string) => string.clone(),
            None => {
                format!("Cursor: {}, {} -- {} lines",
                self.text_field.cursor_x, self.text_field.cursor_y, self.text_field.lines.len())
            }
        }
    }

    fn refresh_screen(&mut self) -> std::io::Result<()> {
        queue!(&mut self.w, terminal::Clear(ClearType::All))?;
        self.print_header()?;
        for i in 2..self.win_size.1-1 {
            queue!(&mut self.w, cursor::MoveTo(0,i), style::Print("~"))?;
            self.text_field.print_line(&mut self.w, (i-2) as usize)?;
        }
        let status_message = self.get_status_message();
        queue!(&mut self.w, cursor::MoveTo(0,self.win_size.1-1), style::Print(status_message.as_str()))?;
        let cursor_position = self.text_field.get_cursor_position();
        queue!(&mut self.w, cursor::MoveTo(cursor_position.0, cursor_position.1), cursor::Show)?;
        self.w.flush()
    }

    fn save(&mut self) -> std::io::Result<()> {
        let default = if let Some(name) = &self.file_name {
            name.clone()
        }else{
            String::with_capacity(32)
        };
        self.file_name = prompt!(self, "Enter a path to save to:", default);
        match &self.file_name {
            Some(name) => self.text_field.save(name)?,
            _ => {},
        }
        Ok(())
    }

    fn load(&mut self) -> std::io::Result<()> {
        let default = if let Some(name) = &self.file_name {
            name.clone()
        }else{
            String::with_capacity(32)
        };
        self.file_name = prompt!(self, "Enter a path to load to:", default);
        match &self.file_name {
            Some(name) => {
                self.text_field.load(name)
            },
            _ => {},
        }
        Ok(())
    }

    fn find_phrase(editor: &mut Editor, input: &String, key_code: KeyCode) {
        editor.text_field.find_phrase(input, key_code);
    }

    fn find(&mut self) -> std::io::Result<()> {
        let _ = prompt!(self, "Find:", String::with_capacity(32), Editor::find_phrase);
        Ok(())
    }

    fn quit(&mut self) -> std::io::Result<()>{
        self.set_status_message(Some(String::from("Press Ctrl-C again to confirm quit. Press Esc to cancel")));
        loop {
            self.refresh_screen()?;
            execute!(&mut self.w, cursor::Hide)?;
            if let KeyEvent {code: c, modifiers: m, kind: KeyEventKind::Press, ..} = read_key()? {
                match (c, m) {
                    (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        self.running = false;
                        break;
                    },
                    (KeyCode::Esc, _) => break,
                    _ => ()
                }
            }
        }
        Ok(())
    }

    fn run(&mut self) -> std::io::Result<()> {
        while self.running {
            self.set_status_message(None);
            self.refresh_screen()?;
            match read_key()? {
                KeyEvent{
                    code: KeyCode::Char('c'),
                    modifiers: event::KeyModifiers::CONTROL,
                    ..
                } => self.quit()?,
                KeyEvent{
                    code: KeyCode::Char('s'),
                    modifiers: event::KeyModifiers::CONTROL,
                    ..
                } => self.save()?,
                KeyEvent{
                    code: KeyCode::Char('l'),
                    modifiers: event::KeyModifiers::CONTROL,
                    ..
                } => self.load()?,
                KeyEvent{
                    code: KeyCode::Char('f'),
                    modifiers: event::KeyModifiers::CONTROL,
                    ..
                } => self.find()?,
                KeyEvent {
                    code: direction @ (KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right),
                    modifiers: event::KeyModifiers::NONE,
                    kind: KeyEventKind::Press,
                    ..
                } => self.text_field.move_cursor(direction),
                KeyEvent {
                    code: code @ (KeyCode::Char(..) | KeyCode::Tab),
                    kind: event::KeyEventKind::Press,
                    ..
                } => self.text_field.insert_char(match code {
                    KeyCode::Tab => '\t',
                    KeyCode::Char(ch) => ch,
                    _ => unreachable!(),
                }),
                KeyEvent {
                    code: KeyCode::Enter,
                    kind: event::KeyEventKind::Press,
                    ..
                } => self.text_field.new_line(),
                KeyEvent {
                    code: KeyCode::Backspace,
                    kind: event::KeyEventKind::Press,
                    ..
                } => self.text_field.delete_char(),
                _ => {}
            }
        }
        execute!(&mut self.w, terminal::Clear(ClearType::All), cursor::MoveTo(0, 0))
    }
}

fn main() -> Result<()> {
    let mut editor = Editor::new();
    editor.run()
}