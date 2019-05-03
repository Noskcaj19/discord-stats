let msg_count = fetch("/api/total_msg_count").then(x => x.json());
let edit_count = fetch("/api/edit_count").then(x => x.json());
let user_msg_count = fetch("/api/msg_count").then(x => x.json());
let channels = fetch("/api/channels").then(x => x.json());
let guilds = fetch("/api/guilds").then(x => x.json());
let msgs_per_day = fetch("/api/user_msg_count_per_day").then(x => x.json()).then(x => x.map(([date, ...x]) => [new Date(date), ...x]))

window.onload = function populate() {

    Promise.all([msg_count, user_msg_count, channels, guilds, msgs_per_day, edit_count]).then(values => {
        let [msg_count, user_msg_count, channels, guilds, msgs_per_day, edit_count] = values
        let stats_textarea = document.getElementById("stats");
        let channel_count = channels.filter(i => i['guild_id']).length
        let dm_count = channels.length - channel_count

        stats_textarea.innerText = `Total logged messages: ${msg_count}
        Users logged messages: ${user_msg_count}
        Logged guilds: ${guilds.length}
        Logged channels: ${channel_count}
        Logged direct message channels: ${dm_count}
        Total edits: ${edit_count}`

        let x_axis = ["x"]
        let user_msgs_pub = ["public"]
        let user_msgs_priv = ["private"]
        for (let day of msgs_per_day) {
            x_axis.push(day[0])

            user_msgs_pub.push(day[1])
            user_msgs_priv.push(day[2])
        }

        c3.generate({
            bindto: '#sent-messages',
            data: {
                x: 'x',
                columns: [
                    x_axis, user_msgs_priv, user_msgs_pub
                ],
                type: 'area',
                names: {
                    "private": "Private Messages",
                    "public": "Public Messages"
                }
            },
            axis: {
                x: {
                    type: 'timeseries',
                    tick: {
                        format: '%Y-%m-%d'
                    }
                }
            },
            title: {
                text: "Messages sent"
            }
        })
    })
}
