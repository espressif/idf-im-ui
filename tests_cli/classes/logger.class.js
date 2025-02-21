import winston from "winston";

const logLevel = process.env.DEBUG === "true" ? "debug" : "info";
const logToFile = process.env.LOG_TO_FILE === "true";

const transports = [
    new winston.transports.Console({
        format: winston.format.combine(
            winston.format.colorize(),
            winston.format.simple()
        ),
    }),
];

if (logToFile) {
    transports.push(
        new winston.transports.File({
            filename: "./test.log",
            format: winston.format.combine(
                winston.format.timestamp(),
                winston.format.json()
            ),
        })
    );
}

const logger = winston.createLogger({
    level: logLevel,
    format: winston.format.combine(
        winston.format.printf(({ level, message }) => {
            return `[${level}]: ${message}`;
        })
    ),
    transports: transports,
});

export default logger;
