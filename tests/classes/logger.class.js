import winston from "winston";

const logToFile = process.env.LOG_TO_FILE === "true";

const transports = [
  new winston.transports.Console({
    level: "info",
    format: winston.format.combine(
      winston.format.colorize(),
      winston.format.simple()
    ),
  }),
];

if (logToFile) {
  const timestamp = new Date().toISOString().replace(/[-:.]/g, "").slice(0, 15);

  transports.push(
    new winston.transports.File({
      level: "debug",
      filename: `./test-${timestamp}.log`,
      format: winston.format.combine(
        winston.format.timestamp(),
        winston.format.json()
      ),
    })
  );
}

const logger = winston.createLogger({
  level: "debug",
  format: winston.format.combine(
    winston.format.printf(({ level, message }) => {
      return `[${level}]: ${message}`;
    })
  ),
  transports: transports,
});

export default logger;
