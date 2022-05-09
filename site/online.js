// <!-- fetch('http://example.com/movies.json') -->
// <!-- .then(response => response.json()) -->
// <!-- .then(data => console.log(data)); -->

const zeroPad = (unit) => String(unit).padStart(2, '0');

const secondsToText = (s) => {
	const second = zeroPad(Math.trunc(s % 60));
	const minute = zeroPad(Math.floor(s / 60 % 60));
	const hour = zeroPad(Math.floor(s / 60 / 60 % 24));
	const day = Math.floor(s / 60 / 60 / 24);
	// return short ?
		// `${day}d, ${hour}h, ${minute}m, ${second}s`;
		return `${day} days, ${hour} hours, ${minute} minutes, ${second} seconds`;
};


async function check_status() {
	console.log("from script");

	let request = await fetch("https://api.adsbdb.com/v0/online");
	let response = await request.json();
	// response.uptime *&& response.api_verion
	console.log(response);
	let uptime = document.querySelector("#uptime");

	// let uptime_formatted =
	uptime.innerHTML =  secondsToText(response.response.uptime)


	let api_version = document.querySelector("#api_version");
	api_version.innerHTML = response.response.api_version

}




check_status()