-- Add migration script here
-- sqlite 3

create table if not exists user (
    id integer primary key,
    username varchar(32) not null unique,
    created integer not null
);

create table if not exists passwd (
    user_id integer primary key,
    algo integer not null,
    options text,
    salt blob not null,
    pass_hash blob not null,
    foreign key (user_id) references user(id)
) without rowid;

create table if not exists game (
    id integer primary key,
    game_name varchar(255) not null
);

create table if not exists game_event (
    id integer primary key,
    event_name varchar(255) not null,
    created integer not null
);

create table if not exists user_submitted_bot (
    id integer primary key,
    bot_hash blob not null,
    created integer not null,
    user_id integer not null,
    game_id integer not null,
    foreign key (user_id) references user(id),
    foreign key (game_id) references game(id),
    constraint user_bot_hash unique (user_id, bot_hash) on conflict rollback
);

create table if not exists game_code (
    id integer primary key,
    code varchar(40)
);

create table if not exists bot_participates_in_event (
    bot_id integer,
    event_id integer,
    foreign key (bot_id) references user_submitted_bot(id),
    foreign key (event_id) references game_event(id),
    primary key (bot_id, event_id) on conflict rollback

) without rowid;

create table if not exists compeated_against (
    id integer primary key,
    in_event integer, -- may not be a part of an event
    primary_bot integer not null,
    secondary_bot integer not null,
    start_time integer not null,
    end_time integer not null,
    end_code_id integer not null,
    foreign key (in_event) references game_event(id),
    foreign key (primary_bot) references user_submitted_bot(id),
    foreign key (secondary_bot) references user_submitted_bot(id),
    foreign key (end_code_id) references game_code(id)
);
