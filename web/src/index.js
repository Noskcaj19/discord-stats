let msg_count = fetch("/api/total_msg_count").then(x => x.json());
let user_msg_count = fetch("/api/msg_count").then(x => x.json());
let channels = fetch("/api/channels").then(x => x.json());
let guilds = fetch("/api/guilds").then(x => x.json());
let msgs_per_day = fetch("/api/user_msg_count_per_day").then(x => x.json()).then(x => x.map(([date, cnt]) => [new Date(date), cnt]))
let total_msgs_per_day = fetch("/api/total_msg_count_per_day").then(x => x.json()).then(x => x.map(([date, cnt]) => [new Date(date), cnt]))

window.onload = function populate() {

    Promise.all([msg_count, user_msg_count, channels, guilds, msgs_per_day, total_msgs_per_day]).then(values => {
        let [msg_count, user_msg_count, channels, guilds, msgs_per_day, total_msgs_per_day] = values
        console.log(msgs_per_day, total_msgs_per_day)
        let stats_textarea = document.getElementById("stats");
        let channel_count = channels.filter(i => i['guild_id']).length
        let dm_count = channels.length - channel_count

        stats_textarea.innerText = `Total logged messages: ${msg_count}
        Users logged messages: ${user_msg_count}
        Logged guilds: ${guilds.length}
        Logged channels: ${channel_count}
        Logged direct message channels: ${dm_count}`

        let x_axis = ["x"]
        let user_msgs = ["Messages you sent"]
        let other_msgs = ["Messages others sent"]

        for (i in msgs_per_day) {
            x_axis.push(msgs_per_day[i][0])
            user_msgs.push(msgs_per_day[i][1])
            other_msgs.push(total_msgs_per_day[i][1] - msgs_per_day[i][1])
        }

        c3.generate({
                bindto: '#vis',
                data: {
                    x: 'x',

                    columns: [
                        x_axis, user_msgs, other_msgs
                    ]
                },
                axis: {
                    x: {
                        type: 'timeseries',
                        tick: {
                            format: '%Y-%m-%d'
                        }
                    }
                }
            })
        ;
    })
}
