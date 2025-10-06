
const aircraft_btn_handler = async () => {
	const request = await fetch(`https://api.adsbdb.com/v0/aircraft/random`);
	const response = await request.json();
	document.querySelector("#random_aircraft").textContent = JSON.stringify(response, null, 2)
}

const random_aircraft_btn = document.getElementById("random_aircraft_btn");
random_aircraft_btn.addEventListener("click", aircraft_btn_handler);

const callsign_btn_handler = async () => {
	const request = await fetch(`https://api.adsbdb.com/v0/callsign/random`);
	const response = await request.json();
	document.querySelector("#random_callsign").textContent = JSON.stringify(response, null, 2)
}

const random_callsign_btn = document.getElementById("random_callsign_btn");
random_callsign_btn.addEventListener("click", callsign_btn_handler);

const airline_btn_handler = async () => {
	const request = await fetch(`https://api.adsbdb.com/v0/airline/random`);
	const response = await request.json();
	document.querySelector("#random_airline").textContent = JSON.stringify(response, null, 2)
}

const random_airline_btn = document.getElementById("random_airline_btn");
random_airline_btn.addEventListener("click", airline_btn_handler);

const zeroPad = (unit) => String(unit).padStart(2, '0');

const secondsToText = (s) => {
	const second = zeroPad(Math.trunc(s % 60));
	const minute = zeroPad(Math.floor(s / 60 % 60));
	const hour = zeroPad(Math.floor(s / 60 / 60 % 24));
	const day = zeroPad(Math.floor(s / 60 / 60 / 24));
	return `${day} days, ${hour} hours, ${minute} minutes, ${second} seconds`;
};

const check_status = async () => {
	try {
		let ms = 0;
		const request = await fetch("https://api.adsbdb.com/v0/online");
		const response = await request.json();
		const uptime = document.querySelector("#uptime");
		const api_version = document.querySelector("#api_version");
		if (response?.response?.uptime && response?.response?.api_version) {
			api_version.innerHTML = response.response.api_version;
			ms = response.response.uptime;
			uptime.innerHTML = secondsToText(ms)
			document.querySelector('#stats').style.display = 'block';
			setInterval(() => {
				ms++;
				uptime.innerHTML = secondsToText(ms);
			}, 1000);
		}
	} catch (e) {
		console.log(e)
	}
}

check_status();