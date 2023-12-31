use crate::page::Page;

pub struct History {
    pages: Vec<Page>,
    head: usize,
}

impl History {
    pub fn new() -> History {
        let mut pages = Vec::new();
        pages.push(Page::new());
        History { pages, head: 0 }
    }

    pub fn current(&self) -> &Page {
        &self.pages[self.head]
    }

    pub fn push(&mut self, page: Page) {
        self.head += 1;
        self.pages.truncate(self.head);
        self.pages.push(page);
    }

    pub fn prev(&mut self) {
        if self.head > 0 {
            self.head -= 1;
        }
    }

    pub fn next(&mut self) {
        if self.head < self.pages.len() - 1 {
            self.head += 1;
        }
    }
}
