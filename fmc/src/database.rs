use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use bevy::prelude::*;
use indexmap::IndexSet;

use crate::{
    blocks::{BlockData, BlockId, BlockState},
    items::ItemId,
    world::chunk::Chunk,
};

/// Sets up the database at startup
pub struct DatabasePlugin {
    pub path: String,
}

impl Default for DatabasePlugin {
    fn default() -> Self {
        Self {
            path: "./world.sqlite".to_owned(),
        }
    }
}

impl Plugin for DatabasePlugin {
    fn build(&self, app: &mut App) {
        let database = Database::new(self.path.clone());

        database.build();
        database.save_block_ids();
        database.save_items();
        database.save_models();
        //    setup_new_world_database(&settings.world_database_path);
        //} else if rusqlite::Connection::open(&settings.world_database_path).is_err() {
        //    panic!("Could not open the world file at '{}', make sure it is the correct file, else it might be corrupt", settings.world_database_path);
        //}

        app.insert_resource(database);
    }
}

/// Database connection manager
#[derive(Resource, Clone)]
pub struct Database(Arc<DatabaseInner>);

impl Database {
    fn path(&self) -> &str {
        &self.0.path
    }
}

// TODO: Two modes, one where it saves only changes to disk and one where it saves all chunk data.
//       Changes are best for single instances that don't care about the cpu load of re-generating
//       chunks. For large servers it is preferable to save cpu at the cost of storage.
// TODO: Implement connection pool
struct DatabaseInner {
    write_connection: Mutex<rusqlite::Connection>,
    path: String,
    //pub pool: Mutex<Vec<rusqlite::Connection>>
}

/// A lock on the single write connection the database has.
#[derive(Deref, DerefMut)]
pub struct WriteConnection<'a> {
    conn: MutexGuard<'a, rusqlite::Connection>,
}

// pub struct ReadConnection {
//     database: Database
//     conn: ...
// }

//impl Drop for Connection {
//    fn drop(&mut self) {
//        self.database.put_back(self.conn);
//    }
//}

// TODO: Extract functions and have them take a connection instead?
impl Database {
    pub fn new(path: String) -> Self {
        let write_connection = rusqlite::Connection::open(&path).unwrap();
        write_connection
            .execute_batch(
                "PRAGMA journal_mode = wal;
                    PRAGMA synchronous = 1;",
            )
            .unwrap();

        return Self(Arc::new(DatabaseInner {
            write_connection: Mutex::new(write_connection),
            path,
        }));
    }

    /// Get the write connection.
    pub fn get_write_connection(&self) -> WriteConnection {
        WriteConnection {
            conn: self.0.write_connection.lock().unwrap(),
        }
    }

    /// Get a connection to read from the database. Note that this is only semantic, you can
    /// use it to write, but that WILL lead to errors. Use `get_write_connection` instead.
    pub fn get_read_connection(&self) -> rusqlite::Connection {
        return rusqlite::Connection::open(self.path()).unwrap();
    }

    pub fn build(&self) {
        let conn = self.get_write_connection();

        //conn.execute("drop table if exists blocks", []).unwrap();
        conn.execute("drop table if exists block_ids", []).unwrap();
        conn.execute("drop table if exists item_ids", []).unwrap();
        conn.execute("drop table if exists model_ids", []).unwrap();
        //conn.execute("drop table if exists players", []).unwrap();
        //conn.execute("drop table if exists storage", []).unwrap();

        // TODO: Test WITHOUT ROWID, it's better maybe.
        // TODO: Test with r*tree, it is already included just need to enable.
        conn.execute(
            "create table if not exists blocks (
                x INTEGER,
                y INTEGER,
                z INTEGER,
                block_id INTEGER,
                block_state INTEGER,
                block_data BLOB,
                PRIMARY KEY (x,y,z)
             )",
            [],
        )
        .expect("Could not create 'blocks' table");

        conn.execute(
            "create table if not exists block_ids (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
                )",
            [],
        )
        .expect("Could not create 'block_ids' table");

        conn.execute(
            "create table if not exists item_ids (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
                )",
            [],
        )
        .expect("Could not create 'item_ids' table");

        conn.execute(
            "create table if not exists model_ids (
                name TEXT NOT NULL UNIQUE,
                id INTEGER
                )",
            [],
        )
        .expect("Could not create 'model_ids' table");

        // All data about a player is stored in the save field. Its format is decided by the program.
        conn.execute(
            "create table if not exists players (
                name TEXT PRIMARY KEY,
                save BLOB NOT NULL
                )",
            [],
        )
        .expect("Could not create 'players' table");

        // General persistent storage
        conn.execute(
            "create table if not exists storage (
                name TEXT PRIMARY KEY,
                data TEXT NOT NULL
                )",
            [],
        )
        .expect("Could not create 'storage' table");
    }

    // TODO: rusqlite doesn't drop stuff correctly so there's all kinds of errors when you don't
    // localize statements.
    //fn _load_chunk(connection: rusqlite::Connection, position: &IVec3) -> (Chunk, usize) {
    //    let mut block_stmt = connection
    //        .prepare(
    //            r#"
    //        select
    //            x, y, z, block_id, block_state
    //        from
    //            blocks
    //        where
    //            (x between ? and ?)
    //        and
    //            (y between ? and ?)
    //        and
    //            (z between ? and ?)
    //        order by
    //            rowid asc"#,
    //        )
    //        .unwrap();

    //    const OFFSET: i32 = CHUNK_SIZE as i32 - 1;
    //    let mut rows = block_stmt
    //        .query([
    //            &position.x,
    //            &(position.x + OFFSET),
    //            &position.y,
    //            &(position.y + OFFSET),
    //            &position.z,
    //            &(position.z + OFFSET),
    //        ])
    //        .unwrap();

    //    let mut chunk = Chunk::new(Blocks::get().get_id("air"));
    //    let mut count = 0;

    //    while let Some(row) = rows.next().unwrap() {
    //        let index = (((row.get::<_, i32>(0).unwrap() & OFFSET) << 8)
    //            & ((row.get::<_, i32>(1).unwrap() & OFFSET) << 4)
    //            & (row.get::<_, i32>(2).unwrap() & OFFSET)) as usize;

    //        chunk.blocks[index] = row.get::<_, BlockId>(3).unwrap();

    //        // TODO: rusqlite supports FromSql for serde_json::Value, but since serde_json has been
    //        // forked.
    //        if let Ok(block_state_ref) = row.get_ref(4) {
    //            match block_state_ref {
    //                rusqlite::types::ValueRef::Blob(bytes) => chunk
    //                    .block_state
    //                    .insert(index, bincode::deserialize(bytes).unwrap()),
    //                _ => panic!("Block state stored as non-blob"),
    //            };
    //        }

    //        count += 1;
    //    }

    //    return (chunk, count);
    //}

    // The blocks table stores three types of chunks
    // 1. both block_state and blocks are NULL, it's an air chunk
    // 2. blocks can be deserialized to a hashmap, it's a partially generated chunk
    // 3. blocks can be deserialized to a vec, it's a fully generated chunk
    //pub async fn load_chunk(&self, position: &IVec3) -> Option<Chunk> {
    //    let conn = self.get_connection();

    //    let (mut chunk, count) = Self::_load_chunk(conn, position);

    //    // The block_state column is abused to reduce the storage space of uniform chunks (air,
    //    // water, etc) down to 1 block's worth. u16::MAX is stored (an otherwise invalid block
    //    // state) to mark them.

    //    if count == CHUNK_SIZE.pow(3) {
    //        return Some(chunk);
    //    } else if count > 0 {
    //        match chunk.block_state.get(&0) {
    //            Some(block_state) if *block_state == u16::MAX => {
    //                if count == 1 {
    //                    chunk.chunk_type = ChunkType::Uniform(chunk.blocks[0]);
    //                    chunk.blocks = Vec::new();
    //                    chunk.block_state = HashMap::new();
    //                    return Some(chunk);
    //                } else {
    //                    // This is a chunk that was previously uniform, but has had blocks inserted
    //                    // into it through adjacent chunk's terrain generation. It needs to be
    //                    // converted to a normal chunk.
    //                    let base_block = chunk.blocks[0];
    //                    chunk.blocks.iter_mut().for_each(|block| {
    //                        if *block == 0 {
    //                            *block = base_block;
    //                        }
    //                    });
    //                    chunk.chunk_type = ChunkType::Normal;
    //                    self.save_chunk(position, &chunk).await;
    //                    return Some(chunk);
    //                }
    //            }
    //            _ => (),
    //        }
    //        chunk.chunk_type = ChunkType::Partial;
    //        return Some(chunk);
    //    } else {
    //        return None;
    //    }
    //}

    pub fn load_chunk_blocks(
        &self,
        position: &IVec3,
    ) -> HashMap<usize, (BlockId, Option<BlockState>, Option<BlockData>)> {
        let conn = self.get_read_connection();

        let mut block_stmt = conn
            .prepare(
                r#"
            select
                x, z, y, block_id, block_state, block_data
            from
                blocks
            where
                (x between ? and ?)
            and
                (y between ? and ?)
            and
                (z between ? and ?)"#,
            )
            .unwrap();

        const OFFSET: i32 = Chunk::SIZE as i32 - 1;
        let mut rows = block_stmt
            .query([
                &position.x,
                &(position.x + OFFSET),
                &position.y,
                &(position.y + OFFSET),
                &position.z,
                &(position.z + OFFSET),
            ])
            .unwrap();

        let mut blocks = HashMap::new();

        while let Some(row) = rows.next().unwrap() {
            let index = (((row.get::<_, i32>(0).unwrap() & OFFSET) << 8)
                | ((row.get::<_, i32>(1).unwrap() & OFFSET) << 4)
                | (row.get::<_, i32>(2).unwrap() & OFFSET)) as usize;

            blocks.insert(
                index,
                (
                    row.get::<_, BlockId>(3).unwrap(),
                    row.get::<_, u16>(4).ok().map(BlockState),
                    row.get::<_, Vec<u8>>(5).ok().map(BlockData),
                ),
            );
        }

        return blocks;
    }

    //pub async fn save_chunk(&self, position: &IVec3, chunk: &Chunk) {
    //    let mut connection = self.get_connection();
    //    let transaction = connection.transaction().unwrap();
    //    // The conflict is so that this chunk's air blocks doesn't overwrite any previously written
    //    // blocks. These come from partial chunks, and should only be overwritten if we have
    //    // something to place in its stead.
    //    let mut stmt = transaction
    //        .prepare_cached(
    //            r#"
    //        insert into
    //            blocks (x,y,z,block_id,block_state)
    //        values
    //            (?,?,?,?,?)
    //        on conflict(x,y,z) do update set
    //            (block_id, block_state) = (excluded.block_id, excluded.block_state)
    //        where
    //            excluded.block_id is not 0"#,
    //        )
    //        .unwrap();

    //    const OFFSET: i32 = CHUNK_SIZE as i32 - 1;
    //    match chunk {
    //        Chunk::Normal { blocks, block_state } => {
    //            for (i, block_id) in blocks.iter().enumerate() {
    //                let x = (i as i32 & OFFSET << 8) >> 8;
    //                let y = (i as i32 & OFFSET << 4) >> 4;
    //                let z = i as i32 & OFFSET;
    //                stmt.execute(rusqlite::params![
    //                    position.x + x,
    //                    position.y + y,
    //                    position.z + z,
    //                    block_id,
    //                    block_state
    //                        .get(&i)
    //                        .map(|state| bincode::serialize(state).ok())
    //                ])
    //                .unwrap();
    //            }
    //        }
    //        Chunk::Partial => {
    //            for (i, block_id) in chunk.blocks.iter().enumerate() {
    //                if *block_id == 0 {
    //                    continue;
    //                }

    //                let x = (i as i32 & OFFSET << 8) >> 8;
    //                let y = (i as i32 & OFFSET << 4) >> 4;
    //                let z = i as i32 & OFFSET;
    //                stmt.execute(rusqlite::params![
    //                    position.x + x,
    //                    position.y + y,
    //                    position.z + z,
    //                    block_id,
    //                    chunk
    //                        .block_state
    //                        .get(&i)
    //                        .map(|state| bincode::serialize(state).ok())
    //                ])
    //                .unwrap();
    //            }
    //        }
    //        Chunk::Uniform(block_id) => {
    //            let x = (0 & OFFSET << 8) >> 8;
    //            let y = (0 & OFFSET << 4) >> 4;
    //            let z = 0 & OFFSET;
    //            stmt.execute(rusqlite::params![
    //                position.x + x,
    //                position.y + y,
    //                position.z + z,
    //                block_id,
    //                u16::MAX,
    //            ])
    //            .unwrap();
    //        }
    //    }

    //    // I have to idea why you have to do this. stmt.finalize() does not work.
    //    drop(stmt);
    //    transaction.commit().unwrap();
    //}

    //pub fn load_player(&self, username: &str) -> Option<PlayerSave> {
    //    let conn = self.get_connection();

    //    let mut stmt = conn
    //        .prepare("SELECT save FROM players WHERE name = ?")
    //        .unwrap();
    //    let mut rows = if let Ok(rows) = stmt.query([username]) {
    //        rows
    //    } else {
    //        return None;
    //    };

    //    if let Some(row) = rows.next().unwrap() {
    //        let bytes: Vec<u8> = row.get(0).unwrap();
    //        let save: PlayerSave = bincode::deserialize(&bytes).unwrap();
    //        return Some(save);
    //    } else {
    //        return None;
    //    };
    //}

    /// Save a player's information
    //pub fn save_player(&self, username: &str, save: &PlayerSave) {
    //    let conn = self.get_connection();

    //    let mut stmt = conn
    //        .prepare("INSERT OR REPLACE INTO players VALUES (?,?)")
    //        .unwrap();
    //    stmt.execute(rusqlite::params![
    //        username,
    //        bincode::serialize(save).unwrap()
    //    ])
    //    .unwrap();
    //}

    /// Add new block ids to the database. The ids will be constant and cannot change.
    pub fn save_block_ids(&self) {
        fn walk_dir<P: AsRef<std::path::Path>>(dir: P) -> Vec<std::path::PathBuf> {
            let mut files = Vec::new();

            let directory = std::fs::read_dir(dir).expect(
                "Could not read files from block configuration directory, make sure it is present.",
            );

            for entry in directory {
                let file_path = entry
                    .expect("Failed to read the filename of a block config")
                    .path();

                if file_path.is_dir() {
                    let sub_files = walk_dir(&file_path);
                    files.extend(sub_files);
                } else {
                    files.push(file_path);
                }
            }

            files
        }

        let mut block_names: Vec<String> = Vec::new();

        for file_path in walk_dir(&crate::blocks::BLOCK_CONFIG_PATH) {
            let file = std::fs::File::open(&file_path).unwrap();
            let config: serde_json::Value = match serde_json::from_reader(file) {
                Ok(c) => c,
                Err(e) => panic!(
                    "Failed to read block config at path: {}\nError: {}",
                    file_path.display(),
                    e
                ),
            };

            let block_name = match config.get("name").and_then(|name| name.as_str()) {
                Some(n) => n,
                // Blocks that don't have names are used as parent blocks and are not saved.
                None => continue,
            };

            block_names.push(block_name.to_owned());
        }

        let mut conn = self.get_write_connection();
        let tx = conn.transaction().unwrap();

        let mut stmt = tx
            .prepare("INSERT INTO block_ids (name) VALUES (?)")
            .unwrap();

        for name in block_names.into_iter() {
            if let Err(e) = stmt.execute(rusqlite::params![name]) {
                panic!("Couldn't write the {name} block to the database, this is most likely because of a duplicate block with the same name.\nError: {e}");
            }
        }

        stmt.finalize().unwrap();
        tx.commit().expect("Failed to update block ids in database");
    }

    pub fn load_block_ids(&self) -> HashMap<String, BlockId> {
        let conn = self.get_read_connection();
        let mut stmt = conn.prepare("SELECT * FROM block_ids").unwrap();
        let mut rows = stmt.query([]).unwrap();

        let mut blocks = HashMap::new();
        while let Some(row) = rows.next().unwrap() {
            blocks.insert(row.get(1).unwrap(), blocks.len() as BlockId);
        }

        return blocks;
    }

    pub fn save_items(&self) {
        let mut item_names = Vec::new();

        let directory = std::fs::read_dir(crate::items::ITEM_CONFIG_PATH).expect(
            "Could not read files from item configuration directory, make sure it is present.\n",
        );

        for dir_entry in directory {
            let file_path = match dir_entry {
                Ok(d) => d.path(),
                Err(e) => panic!(
                    "Failed to read the filename of a block config, Error: {}",
                    e
                ),
            };

            item_names.push(
                file_path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_lowercase(),
            );
        }

        let mut conn = self.get_write_connection();
        let tx = conn.transaction().unwrap();

        let mut stmt = tx
            .prepare("INSERT INTO item_ids (name) VALUES (?)")
            .unwrap();

        for name in item_names {
            stmt.execute(rusqlite::params![name]).unwrap();
        }

        stmt.finalize().unwrap();
        tx.commit()
            .expect("Failed to save item ids to the database");
    }

    pub fn load_item_ids(&self) -> HashMap<String, ItemId> {
        let conn = self.get_read_connection();
        let mut stmt = conn.prepare("SELECT * FROM item_ids").unwrap();
        let mut rows = stmt.query([]).unwrap();

        let mut blocks = HashMap::new();
        while let Some(row) = rows.next().unwrap() {
            blocks.insert(row.get(1).unwrap(), row.get(0).unwrap());
        }

        return blocks;
    }

    pub fn save_models(&self) {
        let mut model_names = Vec::new();

        let directory = std::fs::read_dir(crate::models::MODEL_PATH)
            .expect("Could not read files from model directory, make sure it is present.");

        for dir_entry in directory {
            let file_path = match dir_entry {
                Ok(d) => d.path(),
                Err(e) => panic!("Failed to read the filename of a model, Error: {}", e),
            };

            model_names.push(
                file_path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_lowercase(),
            );
        }

        let mut conn = self.get_write_connection();
        let tx = conn.transaction().unwrap();

        let mut stmt = tx
            .prepare("INSERT INTO model_ids (name) VALUES (?)")
            .unwrap();

        for name in model_names.into_iter() {
            stmt.execute(rusqlite::params![name]).unwrap();
        }

        stmt.finalize().unwrap();
        tx.commit()
            .expect("Failed to save item ids to the database");
    }

    // Load model names sorted by their model ids
    pub fn load_models(&self) -> IndexSet<String> {
        let conn = self.get_read_connection();
        let mut stmt = conn.prepare("SELECT name FROM model_ids").unwrap();
        let mut rows = stmt.query([]).unwrap();

        let mut models = IndexSet::new();
        while let Some(row) = rows.next().unwrap() {
            models.insert(row.get(0).unwrap());
        }

        return models;
    }
}
