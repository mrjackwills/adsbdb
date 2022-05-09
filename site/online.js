const zeroPad = (unit) => String(unit).padStart(2, '0');

const secondsToText = (s) => {
	const second = zeroPad(Math.trunc(s % 60));
	const minute = zeroPad(Math.floor(s / 60 % 60));
	const hour = zeroPad(Math.floor(s / 60 / 60 % 24));
	const day = zeroPad(Math.floor(s / 60 / 60 / 24));
	return `${day} days, ${hour} hours, ${minute} minutes, ${second} seconds`;
};


const  check_status = async () => {
	const request = await fetch("https://api.adsbdb.com/v0/online");
	const response = await request.json();
	const uptime = document.querySelector("#uptime");
	const api_version = document.querySelector("#api_version");

	if (response?.response?.uptime && response?.response?.api_version) {
		api_version.innerHTML = response.response.api_version
		uptime.innerHTML = secondsToText(response.response.uptime)
		document.querySelector('#stats').style.display = 'block';

	}
}

check_status();