use rusqlite::{params, Connection, Result};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct TopicSession {
    pub chat_id: i64,
    pub thread_id: i32,
    pub session_id: String,
    pub cwd: String,
    pub created_at: i64,
}

#[derive(Clone)]
pub struct SqliteStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStore {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS topic_sessions (
                chat_id    INTEGER NOT NULL,
                thread_id  INTEGER NOT NULL,
                session_id TEXT NOT NULL,
                cwd        TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (chat_id, thread_id)
            );"
        )?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn get(&self, chat_id: i64, thread_id: i32) -> Option<TopicSession> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT chat_id, thread_id, session_id, cwd, created_at
             FROM topic_sessions WHERE chat_id=?1 AND thread_id=?2",
            params![chat_id, thread_id],
            |row| Ok(TopicSession {
                chat_id:    row.get(0)?,
                thread_id:  row.get(1)?,
                session_id: row.get(2)?,
                cwd:        row.get(3)?,
                created_at: row.get(4)?,
            }),
        ).ok()
    }

    pub fn save(&self, session: &TopicSession) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO topic_sessions (chat_id, thread_id, session_id, cwd, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(chat_id, thread_id) DO UPDATE SET
               session_id=excluded.session_id,
               updated_at=excluded.updated_at",
            params![session.chat_id, session.thread_id, session.session_id, session.cwd, session.created_at, now],
        )?;
        Ok(())
    }

    pub fn list_all(&self) -> Vec<TopicSession> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT chat_id, thread_id, session_id, cwd, created_at FROM topic_sessions"
        ).unwrap();
        stmt.query_map([], |row| Ok(TopicSession {
            chat_id:    row.get(0)?,
            thread_id:  row.get(1)?,
            session_id: row.get(2)?,
            cwd:        row.get(3)?,
            created_at: row.get(4)?,
        })).unwrap()
            .filter_map(|r| r.ok())
            .collect()
    }
}
