use super::*;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

fn db_error(message: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.into())
}

fn sql<T>(
    db: &Database,
    action: impl FnOnce(&mut Connection) -> rusqlite::Result<T>,
) -> Result<T, String> {
    db.with_connection(action)
        .map_err(|error| error.to_string())
}

fn decode(text: &str) -> rusqlite::Result<Task> {
    serde_json::from_str(text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}

fn encoded(task: &Task) -> rusqlite::Result<String> {
    let value = serde_json::to_string(task)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    if value.len() > 512 * 1024 {
        return Err(db_error(
            "任务记录达到 512 KiB 容量，请结束此任务并新建任务",
        ));
    }
    Ok(value)
}

pub(super) fn initialize(db: &Database) -> Result<(), String> {
    sql(db, |connection| {
        connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS assistant_tasks (
          task_id TEXT PRIMARY KEY, account_uin TEXT NOT NULL, revision INTEGER NOT NULL,
          state TEXT NOT NULL, run_id TEXT, lease_until INTEGER NOT NULL DEFAULT 0,
          updated_at INTEGER NOT NULL, config_fingerprint TEXT NOT NULL, task_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS assistant_tasks_account_updated ON assistant_tasks(account_uin,updated_at DESC);"
    )
    })
}

fn read(connection: &Connection, account: &str, id: &str) -> rusqlite::Result<Task> {
    let value: Option<String> = connection
        .query_row(
            "SELECT task_json FROM assistant_tasks WHERE task_id=?1 AND account_uin=?2",
            params![id, account],
            |row| row.get(0),
        )
        .optional()?;
    decode(&value.ok_or_else(|| db_error("任务不存在或不属于当前账号"))?)
}

pub(super) fn get(db: &Database, account: &str, id: &str) -> Result<Task, String> {
    sql(db, |connection| read(connection, account, id))
}

pub(super) fn list(db: &Database, account: &str) -> Result<Vec<Task>, String> {
    sql(db, |connection| {
        let mut statement=connection.prepare("SELECT task_json FROM assistant_tasks WHERE account_uin=?1 ORDER BY updated_at DESC LIMIT 100")?;
        let texts = statement
            .query_map([account], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        texts.into_iter().map(|text| decode(&text)).collect()
    })
}

pub(super) fn insert(db: &Database, task: &Task, fingerprint: &str) -> Result<(), String> {
    sql(db, |connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let count: usize = transaction.query_row(
            "SELECT COUNT(*) FROM assistant_tasks WHERE account_uin=?1",
            [&task.account_uin],
            |row| row.get(0),
        )?;
        if count >= MAX_TASKS {
            return Err(db_error("当前账号最多保留 100 个任务，请先删除已结束任务"));
        }
        transaction.execute("INSERT INTO assistant_tasks(task_id,account_uin,revision,state,updated_at,config_fingerprint,task_json) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![task.id,task.account_uin,task.revision,task.state,task.updated_at,fingerprint,encoded(task)?])?;
        transaction.commit()
    })
}

fn save(
    connection: &Connection,
    task: &Task,
    lease: Option<i64>,
    fingerprint: Option<&str>,
) -> rusqlite::Result<()> {
    let run_id = task
        .active_run
        .as_ref()
        .filter(|run| run.state == "running")
        .map(|run| run.id.as_str());
    connection.execute("UPDATE assistant_tasks SET revision=?3,state=?4,run_id=?5,updated_at=?6,task_json=?7,lease_until=COALESCE(?8,lease_until),config_fingerprint=COALESCE(?9,config_fingerprint) WHERE task_id=?1 AND account_uin=?2",
        params![task.id,task.account_uin,task.revision,task.state,run_id,task.updated_at,encoded(task)?,lease,fingerprint])?;
    Ok(())
}

pub(super) fn claim(
    db: &Database,
    account: &str,
    id: &str,
    input: &str,
    expected: Option<u64>,
    config: &AiRuntimeConfig,
) -> Result<Task, String> {
    let run_id = new_id("run")?;
    let session_id = new_id("session")?;
    sql(db, |connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut task = read(&transaction, account, id)?;
        if expected.is_some_and(|revision| revision != task.revision) {
            return Err(db_error("任务版本已变化，请刷新后重试"));
        }
        if task.state == "running" {
            return Err(db_error("该任务已由其他窗口或进程运行"));
        }
        if task.messages.len() >= 198 || task.steps.len() >= 120 {
            return Err(db_error("任务会话达到容量，请结束此任务并新建任务"));
        }
        let old_fingerprint: String = transaction.query_row(
            "SELECT config_fingerprint FROM assistant_tasks WHERE task_id=?1",
            [id],
            |row| row.get(0),
        )?;
        let next_fingerprint = fingerprint(config);
        if old_fingerprint != next_fingerprint {
            task.session = TaskSession {
                id: session_id,
                generation: task.session.generation + 1,
                provider: config.provider.clone(),
                model: config.model.clone(),
            };
        }
        let sequence = task
            .active_run
            .as_ref()
            .map(|run| run.sequence + 1)
            .unwrap_or(1);
        let started = now_ms();
        task.active_run = Some(TaskRun {
            id: run_id,
            sequence,
            state: "running".into(),
            input_revision: task.revision,
            persona_revision: 0,
            context_snapshot: Vec::new(),
            started_at: started,
            finished_at: None,
            model_calls: 0,
            max_model_calls: MAX_MODEL_CALLS,
            max_duration_seconds: MAX_DURATION_SECONDS,
        });
        task.messages.push(TaskMessage {
            role: "user".into(),
            content: if input.is_empty() {
                task.instruction.clone()
            } else {
                input.into()
            },
        });
        task.revision += 1;
        task.updated_at = started;
        task.state = "running".into();
        task.error = None;
        task.result = None;
        task.candidate_reply = None;
        save(
            &transaction,
            &task,
            Some(started + (MAX_DURATION_SECONDS as i64) * 1000 + 3000),
            Some(&next_fingerprint),
        )?;
        transaction.commit()?;
        Ok(task)
    })
}

pub(super) fn update_run(
    db: &Database,
    account: &str,
    id: &str,
    run_id: &str,
    update: impl FnOnce(&mut Task) -> Result<(), String>,
) -> Result<Task, String> {
    sql(db, |connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut task = read(&transaction, account, id)?;
        if task.state != "running"
            || task
                .active_run
                .as_ref()
                .is_none_or(|run| run.id != run_id || run.state != "running")
        {
            return Err(db_error("任务运行已取消、中断或被新版本取代"));
        }
        update(&mut task).map_err(|error| db_error(&error))?;
        task.revision += 1;
        task.updated_at = now_ms();
        save(&transaction, &task, None, None)?;
        transaction.commit()?;
        Ok(task)
    })
}

pub(super) fn cancel(db: &Database, account: &str, id: &str) -> Result<Task, String> {
    sql(db, |connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut task = read(&transaction, account, id)?;
        if task.state == "running" || task.state == "ready" {
            task.state = "cancelled".into();
            task.revision += 1;
            task.updated_at = now_ms();
            task.error = Some("用户已取消任务，未发送任何消息".into());
            if let Some(run) = task.active_run.as_mut() {
                run.state = "cancelled".into();
                run.finished_at = Some(task.updated_at);
            }
            save(&transaction, &task, Some(0), None)?;
        }
        transaction.commit()?;
        Ok(task)
    })
}

pub(super) fn delete(db: &Database, account: &str, id: &str) -> Result<(), String> {
    sql(db, |connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if read(&transaction, account, id)?.state == "running" {
            return Err(db_error("请先取消正在运行的任务"));
        }
        transaction.execute(
            "DELETE FROM assistant_tasks WHERE task_id=?1 AND account_uin=?2",
            params![id, account],
        )?;
        transaction.commit()
    })
}

pub(super) fn recover_expired(db: &Database, account: &str, now: i64) -> Result<(), String> {
    sql(db, |connection| {
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let ids = {
            let mut statement=transaction.prepare("SELECT task_id FROM assistant_tasks WHERE account_uin=?1 AND state='running' AND lease_until<?2")?;
            let rows = statement
                .query_map(params![account, now], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        for id in ids {
            let mut task = read(&transaction, account, &id)?;
            task.state = "interrupted".into();
            task.revision += 1;
            task.updated_at = now;
            task.error = Some("上次运行已中断；已保留来源与待办，可手动继续任务".into());
            if let Some(run) = task.active_run.as_mut() {
                run.state = "interrupted".into();
                run.finished_at = Some(now);
            }
            save(&transaction, &task, Some(0), None)?;
        }
        transaction.commit()
    })
}
