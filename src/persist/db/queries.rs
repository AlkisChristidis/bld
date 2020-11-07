#![allow(dead_code)]

pub static CREATE_TABLE_PIPELINE_QUERY: &str = r"
    create table pipeline (
        id nvarchar(50) primary key not null,
        name nvarchar(250) not null,
        running boolean
    )
";

pub static SELECT_PIPELINE_BY_ID_QUERY: &str = r"
    select * 
    from pipeline 
    where id = ? 
";

pub static INSERT_PIPELINE_QUERY: &str = r"
    insert into pipeline
    values (?, ?, ?)
";

pub static UPDATE_PIPELINE_QUERY: &str = r"
    update pipeline 
    set running = ? 
    where id = ?
";
