let msg_count = fetch("/api/msg_count").then(x => x.json());
let channels = fetch("/api/channels").then(x => x.json());
let guilds = fetch("/api/guilds").then(x => x.json());


window.onload = function populate() {
    Promise.all([msg_count, channels, guilds]).then(values => {
        let [msg_count, channels, guilds] = values
        let stats_textarea = document.getElementById("stats");
        let channel_count = channels.filter(i => i['guild_id']).length
        let dm_count = channels.length - channel_count

        stats_textarea.innerText = `Total Messages: ${msg_count['count']}
        Logged guilds: ${guilds.length}
        Logged channels: ${channel_count}
        Logged direct messages: ${dm_count}`
    })
}
